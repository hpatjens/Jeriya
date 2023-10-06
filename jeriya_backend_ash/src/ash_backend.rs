use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
};

use crate::{
    ash_immediate::{AshImmediateCommandBufferBuilderHandler, AshImmediateCommandBufferHandler},
    backend_shared::BackendShared,
    presenter::{Presenter, PresenterEvent},
};
use base::{
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    command_pool::{CommandPool, CommandPoolCreateFlags},
    queue_plan::QueuePlan,
    shader_interface,
};
use jeriya_backend::{
    elements::rigid_mesh::RigidMesh,
    gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation},
    immediate::{self, ImmediateRenderingFrame},
    resources::{
        mesh_attributes::{MeshAttributes, MeshAttributesGpuState},
        mesh_attributes_group::MeshAttributesEvent,
    },
    resources::{ResourceEvent, ResourceReceiver},
    rigid_mesh_instance::RigidMeshInstance,
    transactions::{self, PushEvent, Transaction, TransactionProcessor},
    Backend, Camera, CameraContainerGuard, ImmediateCommandBufferBuilderHandler,
};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    instance::Instance,
    physical_device::PhysicalDevice,
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_macros::profile;
use jeriya_shared::{
    debug_info,
    log::{error, info},
    nalgebra::Vector4,
    tracy_client::{span, Client},
    winit::window::WindowId,
    AsDebugInfo, DebugInfo, Handle, RendererConfig, WindowConfig,
};

pub struct AshBackend {
    presenters: HashMap<WindowId, Presenter>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    backend_shared: Arc<BackendShared>,
}

impl ResourceReceiver for AshBackend {
    fn sender(&self) -> &Sender<ResourceEvent> {
        &self.backend_shared.resource_event_sender
    }
}

impl TransactionProcessor for AshBackend {
    fn process(&self, transaction: Transaction) {
        for (index, presenter) in self.presenters.values().enumerate() {
            if index == self.presenters.len() - 1 {
                // Don' clone the last transaction
                presenter.send(PresenterEvent::ProcessTransaction(transaction));
                break;
            }
            presenter.send(PresenterEvent::ProcessTransaction(transaction.clone()));
        }
    }
}

impl AllocateGpuIndex<RigidMesh> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<RigidMesh>> {
        self.backend_shared.rigid_mesh_gpu_index_allocator.lock().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<RigidMesh>) {
        self.backend_shared
            .rigid_mesh_gpu_index_allocator
            .lock()
            .free_gpu_index(gpu_index_allocation);
    }
}

impl AllocateGpuIndex<RigidMeshInstance> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<RigidMeshInstance>> {
        self.backend_shared
            .rigid_mesh_instance_gpu_index_allocator
            .lock()
            .allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<RigidMeshInstance>) {
        self.backend_shared
            .rigid_mesh_instance_gpu_index_allocator
            .lock()
            .free_gpu_index(gpu_index_allocation);
    }
}

impl AllocateGpuIndex<MeshAttributes> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<MeshAttributes>> {
        self.backend_shared.mesh_attributes_gpu_index_allocator.lock().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<MeshAttributes>) {
        self.backend_shared
            .mesh_attributes_gpu_index_allocator
            .lock()
            .free_gpu_index(gpu_index_allocation);
    }
}

#[profile]
impl Backend for AshBackend {
    type BackendConfig = Config;

    type ImmediateCommandBufferBuilderHandler = AshImmediateCommandBufferBuilderHandler;
    type ImmediateCommandBufferHandler = AshImmediateCommandBufferHandler;

    fn new(
        renderer_config: RendererConfig,
        backend_config: Self::BackendConfig,
        window_configs: &[WindowConfig],
    ) -> jeriya_backend::Result<Arc<Self>>
    where
        Self: Sized,
    {
        if window_configs.is_empty() {
            return Err(jeriya_backend::Error::ExpectedWindow);
        }

        info!("Creating Vulkan Entry");
        let entry = Entry::new()?;

        info!("Creating Vulkan Instance");
        let application_name = renderer_config
            .application_name
            .clone()
            .unwrap_or(env!("CARGO_PKG_NAME").to_owned());
        let instance = Instance::new(
            &entry,
            &application_name,
            matches!(backend_config.validation_layer, ValidationLayerConfig::Enabled { .. }),
        )?;

        let validation_layer_callback = match backend_config.validation_layer {
            ValidationLayerConfig::Disabled => {
                info!("Skipping validation layer callback setup");
                None
            }
            ValidationLayerConfig::Enabled { panic_on_message } => {
                info!("Setting up validation layer callback");
                set_panic_on_message(panic_on_message);
                Some(ValidationLayerCallback::new(&entry, &instance)?)
            }
        };

        let windows = window_configs
            .iter()
            .map(|config| (config.window.id(), config.window))
            .collect::<HashMap<_, _>>();
        let surfaces = windows
            .into_iter()
            .map(|(window_id, window)| {
                info!("Creating Surface for window {window_id:?}");
                let surface = Surface::new(&entry, &instance, window)?;
                Ok((window_id, surface))
            })
            .collect::<base::Result<HashMap<WindowId, Arc<Surface>>>>()?;

        info!("Creating PhysicalDevice");
        let physical_device = PhysicalDevice::new(&instance)?;

        info!("Creating QueueSelection");
        let queue_plan = QueuePlan::new(&instance, &physical_device, surfaces.iter())?;

        info!("Creating Device");
        let device = Device::new(physical_device, &instance, queue_plan)?;

        let (resource_event_sender, resource_event_receiver) = mpsc::channel();

        let backend_shared = Arc::new(BackendShared::new(&device, &Arc::new(renderer_config), resource_event_sender)?);

        let presenters = surfaces
            .iter()
            .zip(window_configs)
            .enumerate()
            .map(|(presenter_index, ((window_id, surface), window_config))| {
                info!("Creating presenter for window {window_id:?}");
                let presenter = Presenter::new(
                    presenter_index,
                    *window_id,
                    backend_shared.clone(),
                    window_config.frame_rate,
                    surface,
                )?;
                Ok((*window_id, presenter))
            })
            .collect::<jeriya_backend::Result<HashMap<_, _>>>()?;

        let backend = Arc::new(Self {
            _entry: entry,
            _instance: instance,
            _surfaces: surfaces,
            _validation_layer_callback: validation_layer_callback,
            presenters,
            backend_shared,
        });

        info!("Creating resource thread");
        let backend2 = backend.clone();
        thread::spawn(move || {
            let client = Client::start();
            client.set_thread_name("resource_thread");

            if let Err(err) = run_resource_thread(resource_event_receiver, &backend2) {
                error!("Failed to run resource thread: {err:?}");
            }
        });

        Ok(backend)
    }

    fn create_immediate_command_buffer_builder(
        &self,
        debug_info: DebugInfo,
    ) -> jeriya_backend::Result<immediate::CommandBufferBuilder<Self>> {
        let command_buffer_builder = AshImmediateCommandBufferBuilderHandler::new(self, debug_info)?;
        Ok(immediate::CommandBufferBuilder::new(command_buffer_builder))
    }

    fn render_immediate_command_buffer(
        &self,
        immediate_rendering_frame: &ImmediateRenderingFrame,
        command_buffer: Arc<immediate::CommandBuffer<Self>>,
    ) -> jeriya_backend::Result<()> {
        for presenter in self.presenters.values() {
            presenter.send(PresenterEvent::RenderImmediateCommandBuffer {
                immediate_command_buffer_handler: AshImmediateCommandBufferHandler {
                    commands: command_buffer.command_buffer().commands.clone(),
                    debug_info: command_buffer.command_buffer().debug_info.clone(),
                },
                immediate_rendering_frame: immediate_rendering_frame.clone(),
            });
        }
        Ok(())
    }

    fn cameras(&self) -> CameraContainerGuard {
        CameraContainerGuard::new(
            self.backend_shared.camera_event_queue.lock(),
            self.backend_shared.cameras.lock(),
            self.backend_shared.renderer_config.clone(),
        )
    }

    fn set_active_camera(&self, window_id: WindowId, handle: Handle<Camera>) -> jeriya_backend::Result<()> {
        let presenter = self
            .presenters
            .get(&window_id)
            .ok_or(jeriya_backend::Error::UnknownWindowId(window_id))?;
        presenter.set_active_camera(handle);
        Ok(())
    }

    fn active_camera(&self, window_id: WindowId) -> jeriya_backend::Result<Handle<Camera>> {
        self.presenters
            .get(&window_id)
            .ok_or(jeriya_backend::Error::UnknownWindowId(window_id))
            .map(|presenter| presenter.active_camera())
    }
}

fn run_resource_thread(resource_event_receiver: Receiver<ResourceEvent>, backend: &Arc<AshBackend>) -> jeriya_backend::Result<()> {
    loop {
        let Ok(resource_event) = resource_event_receiver.recv() else {
            panic!("failed to receive frame start");
        };

        let backend_shared = &backend.backend_shared;

        let queue_poll_span = span!("Poll queues");
        let mut queues = backend_shared.queue_scheduler.queues();
        queues.transfer_queue().poll_completed_fences()?;
        drop(queues);
        drop(queue_poll_span);

        match resource_event {
            ResourceEvent::FrameStart => {}
            ResourceEvent::MeshAttributes(mesh_attributes_events) => {
                handle_mesh_attributes_events(backend, mesh_attributes_events)?;
            }
        }
    }
}

#[profile]
fn handle_mesh_attributes_events(
    backend: &Arc<AshBackend>,
    mesh_attributes_events: Vec<MeshAttributesEvent>,
) -> jeriya_backend::Result<()> {
    let _span = span!("Handle mesh attributes events");

    let backend_shared = &backend.backend_shared;

    info!("Creating CommandPool");
    let mut queues = backend_shared.queue_scheduler.queues();
    let command_pool = CommandPool::new(
        &backend_shared.device,
        queues.transfer_queue(),
        CommandPoolCreateFlags::ResetCommandBuffer,
        debug_info!("MeshAttributes-CommandPool"),
    )?;
    drop(queues);

    // Create a new command buffer for maintaining the meshes
    let mut command_buffer = CommandBuffer::new(
        &backend_shared.device,
        &command_pool.clone(),
        debug_info!("MeshAttributes-CommandBuffer"),
    )?;
    let mut command_buffer_builder = CommandBufferBuilder::new(&backend_shared.device, &mut command_buffer)?;
    command_buffer_builder.begin_command_buffer_for_one_time_submit()?;

    // Handle mesh attributes events
    for mesh_attributes_event in mesh_attributes_events {
        match mesh_attributes_event {
            MeshAttributesEvent::Insert { handle, mesh_attributes } => {
                let _span = span!("Insert mesh attributes");

                // Upload the vertex positions to the GPU
                let vertex_positions4 = mesh_attributes
                    .vertex_positions()
                    .iter()
                    .map(|v| Vector4::new(v.x, v.y, v.z, 1.0))
                    .collect::<Vec<_>>();
                let vertex_positions_start_offset = backend_shared
                    .static_vertex_position_buffer
                    .lock()
                    .push(&vertex_positions4, &mut command_buffer_builder)?;

                // Upload the vertex normals to the GPU
                let vertex_normals4 = mesh_attributes
                    .vertex_normals()
                    .iter()
                    .map(|v| Vector4::new(v.x, v.y, v.z, 1.0))
                    .collect::<Vec<_>>();
                let vertex_normals_start_offset = backend_shared
                    .static_vertex_normals_buffer
                    .lock()
                    .push(&vertex_normals4, &mut command_buffer_builder)?;

                // Upload the indices to the GPU
                let indices_start_offset = if let Some(indices) = &mesh_attributes.indices() {
                    backend_shared
                        .static_indices_buffer
                        .lock()
                        .push(&indices, &mut command_buffer_builder)?
                } else {
                    0
                };

                // Upload the MeshAttributes to the GPU
                let vertex_positions_start_offset = vertex_positions_start_offset as u64;
                let vertex_positions_len = mesh_attributes.vertex_positions().len() as u64;
                let vertex_normals_start_offset = vertex_normals_start_offset as u64;
                let vertex_normals_len = mesh_attributes.vertex_normals().len() as u64;
                let indices_start_offset = indices_start_offset as u64;
                let indices_len = mesh_attributes.indices().map(|indices| indices.len() as u64).unwrap_or(0);
                let mesh_attributes_gpu = shader_interface::MeshAttributes {
                    vertex_positions_start_offset,
                    vertex_positions_len,
                    indices_start_offset,
                    indices_len,
                    vertex_normals_start_offset,
                    vertex_normals_len,
                };
                info!("Inserting a new MeshAttributes: {mesh_attributes_gpu:#?}",);
                backend_shared
                    .mesh_attributes_buffer
                    .lock()
                    .set_memory_unaligned_index(mesh_attributes.gpu_index_allocation().index(), &mesh_attributes_gpu)?;

                // Insert the GPU state for the MeshAttributes when the upload to the GPU is done
                let mesh_attributes_gpu_states2 = backend_shared.mesh_attributes_gpu_states.clone();
                let backend2 = backend.clone();
                command_buffer_builder.push_finished_operation(Box::new(move || {
                    mesh_attributes_gpu_states2
                        .lock()
                        .insert(handle.clone(), MeshAttributesGpuState::Uploaded);

                    // Notify the frames that the MeshAttributes are ready
                    let mut transaction = Transaction::new();
                    transaction.push_event(transactions::Event::SetMeshAttributeActive {
                        gpu_index_allocation: *mesh_attributes.gpu_index_allocation(),
                        is_active: true,
                    });
                    backend2.process(transaction);

                    info!(
                        "Upload of MeshAttributes {} ({:?}) to GPU is done",
                        mesh_attributes.as_debug_info().format_one_line(),
                        handle
                    );
                    Ok(())
                }));
            }
        }
    }
    command_buffer_builder.end_command_buffer()?;

    let mut queues = backend_shared.queue_scheduler.queues();
    queues.transfer_queue().submit(command_buffer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod backend_new {
        use jeriya_shared::FrameRate;
        use jeriya_test::create_window;

        use super::*;

        #[test]
        fn smoke() {
            let window = create_window();
            let renderer_config = RendererConfig {
                application_name: Some("my_application".to_owned()),
                ..RendererConfig::default()
            };
            let backend_config = Config::default();
            let window_config = WindowConfig {
                window: &window,
                frame_rate: FrameRate::Unlimited,
            };
            AshBackend::new(renderer_config, backend_config, &[window_config]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            let window_config = WindowConfig {
                window: &window,
                frame_rate: FrameRate::Unlimited,
            };
            AshBackend::new(renderer_config, backend_config, &[window_config]).unwrap();
        }

        #[test]
        fn empty_windows_none() {
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            assert!(matches!(
                AshBackend::new(renderer_config, backend_config, &[]),
                Err(jeriya_backend::Error::ExpectedWindow)
            ));
        }
    }
}
