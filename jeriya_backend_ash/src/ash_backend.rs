use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
};

use crate::{
    ash_immediate::{AshImmediateCommandBufferBuilderHandler, AshImmediateCommandBufferHandler},
    backend_shared::BackendShared,
    presenter::Presenter,
};
use base::{
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    command_pool::{CommandPool, CommandPoolCreateFlags},
    queue::{Queue, QueueType},
    shader_interface,
};
use jeriya_backend::{
    immediate,
    inanimate_mesh::{InanimateMeshEvent, InanimateMeshGpuState, InanimateMeshGroup},
    model::ModelGroup,
    Backend, Camera, CameraContainerGuard, ImmediateCommandBufferBuilderHandler, InanimateMeshInstanceContainerGuard,
    ModelInstanceContainerGuard,
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
    log::{error, info, trace},
    nalgebra::Vector4,
    parking_lot::Mutex,
    tracy_client::{span, Client},
    winit::window::{Window, WindowId},
    AsDebugInfo, DebugInfo, Handle, RendererConfig,
};

#[derive(Debug)]
pub struct ImmediateRenderingRequest {
    pub immediate_command_buffer: AshImmediateCommandBufferHandler,
    pub count: usize,
}

pub struct AshBackend {
    presenters: HashMap<WindowId, Arc<Mutex<Presenter>>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    backend_shared: Arc<BackendShared>,
    frame_start_sender: mpsc::Sender<()>,
}

#[profile]
impl Backend for AshBackend {
    type BackendConfig = Config;

    type ImmediateCommandBufferBuilderHandler = AshImmediateCommandBufferBuilderHandler;
    type ImmediateCommandBufferHandler = AshImmediateCommandBufferHandler;

    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> jeriya_backend::Result<Self>
    where
        Self: Sized,
    {
        if windows.is_empty() {
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

        let windows = windows.iter().map(|window| (window.id(), window)).collect::<HashMap<_, _>>();
        let surfaces = windows
            .iter()
            .map(|(window_id, window)| {
                info!("Creating Surface for window {window_id:?}");
                let surface = Surface::new(&entry, &instance, window)?;
                Ok((*window_id, surface))
            })
            .collect::<base::Result<HashMap<WindowId, Arc<Surface>>>>()?;

        info!("Creating PhysicalDevice");
        let physical_device = PhysicalDevice::new(&instance, surfaces.values())?;

        info!("Creating Device");
        let device = Device::new(physical_device, &instance)?;

        let backend_shared = Arc::new(BackendShared::new(&device, &Arc::new(renderer_config))?);

        info!("Creating inanimate mesh events thread");
        let (frame_start_sender, receiver) = mpsc::channel();
        let backend_shared2 = backend_shared.clone();
        thread::spawn(move || {
            let client = Client::start();
            client.set_thread_name("inanimate_mesh_events_thread");

            if let Err(err) = run_inanimate_mesh_events_thread(receiver, &backend_shared2) {
                error!("Failed to run inanimate mesh events thread: {err:?}");
            }
        });

        let presenters = surfaces
            .iter()
            .enumerate()
            .map(|(presenter_index, (window_id, surface))| {
                info!("Creating presenter for window {window_id:?}");
                let presenter = Presenter::new(presenter_index, window_id, surface, backend_shared.clone())?;
                Ok((*window_id, Arc::new(Mutex::new(presenter))))
            })
            .collect::<jeriya_backend::Result<HashMap<_, _>>>()?;

        Ok(Self {
            _entry: entry,
            _instance: instance,
            _surfaces: surfaces,
            _validation_layer_callback: validation_layer_callback,
            presenters,
            backend_shared,
            frame_start_sender,
        })
    }

    fn handle_window_resized(&self, window_id: WindowId) -> jeriya_backend::Result<()> {
        let presenter = self
            .presenters
            .get(&window_id)
            .ok_or_else(|| base::Error::UnknownWindowId(window_id))?
            .lock();
        presenter.recreate()?;
        Ok(())
    }

    fn handle_render_frame(&self) -> jeriya_backend::Result<()> {
        let _span = span!("AshBackend::handle_render_frame");

        self.frame_start_sender.send(()).expect("failed to send frame start");

        // Render on all surfaces
        for (_window_id, presenter) in &self.presenters {
            let presenter = &mut *presenter.lock();
            presenter.request_frame()?;
        }

        Client::running().expect("client must be running").frame_mark();

        Ok(())
    }

    fn create_immediate_command_buffer_builder(
        &self,
        debug_info: DebugInfo,
    ) -> jeriya_backend::Result<immediate::CommandBufferBuilder<Self>> {
        let command_buffer_builder = AshImmediateCommandBufferBuilderHandler::new(self, debug_info)?;
        Ok(immediate::CommandBufferBuilder::new(command_buffer_builder))
    }

    fn render_immediate_command_buffer(&self, command_buffer: Arc<immediate::CommandBuffer<Self>>) -> jeriya_backend::Result<()> {
        for presenter in self.presenters.values() {
            let immediate_rendering_request = ImmediateRenderingRequest {
                immediate_command_buffer: AshImmediateCommandBufferHandler {
                    commands: command_buffer.command_buffer().commands.clone(),
                    debug_info: command_buffer.command_buffer().debug_info.clone(),
                },
                count: 1,
            };
            presenter.lock().render_immediate_command_buffer(immediate_rendering_request);
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

    fn inanimate_meshes(&self) -> &InanimateMeshGroup {
        &self.backend_shared.inanimate_mesh_group
    }

    fn inanimate_mesh_instances(&self) -> InanimateMeshInstanceContainerGuard {
        InanimateMeshInstanceContainerGuard::new(
            self.backend_shared.inanimate_mesh_instance_event_queue.lock(),
            self.backend_shared.inanimate_mesh_instances.lock(),
            self.backend_shared.renderer_config.clone(),
        )
    }

    fn models(&self) -> &ModelGroup {
        &self.backend_shared.model_group
    }

    fn model_instances(&self) -> ModelInstanceContainerGuard {
        ModelInstanceContainerGuard::new(
            self.backend_shared.model_instance_event_queue.lock(),
            self.backend_shared.model_instances.lock(),
        )
    }

    fn set_active_camera(&self, window_id: WindowId, handle: Handle<Camera>) -> jeriya_backend::Result<()> {
        let presenter = self
            .presenters
            .get(&window_id)
            .ok_or(jeriya_backend::Error::UnknownWindowId(window_id))?;
        presenter.lock().set_active_camera(handle);
        Ok(())
    }

    fn active_camera(&self, window_id: WindowId) -> jeriya_backend::Result<Handle<Camera>> {
        self.presenters
            .get(&window_id)
            .ok_or(jeriya_backend::Error::UnknownWindowId(window_id))
            .map(|presenter| presenter.lock().active_camera())
    }
}

fn run_inanimate_mesh_events_thread(frame_start_receiver: Receiver<()>, backend_shared: &BackendShared) -> jeriya_backend::Result<()> {
    info!("Creating Queue");
    let mut queue = Queue::new(&backend_shared.device, QueueType::Presentation, 0)?;

    loop {
        let Ok(()) = frame_start_receiver.recv() else {
            panic!("failed to receive frame start");
        };
        handle_events(&mut queue, backend_shared)?;
    }
}

#[profile]
fn handle_events(queue: &mut Queue, backend_shared: &BackendShared) -> jeriya_backend::Result<()> {
    if !backend_shared.inanimate_mesh_event_queue.lock().is_empty() {
        let _span = span!("Handle inanimate mesh events");

        info!("Creating CommandPool");
        let command_pool = CommandPool::new(
            &backend_shared.device,
            queue,
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;

        // Create a new command buffer for maintaining the meshes
        let mut command_buffer = CommandBuffer::new(
            &backend_shared.device,
            &command_pool.clone(),
            debug_info!("maintanance-CommandBuffer"),
        )?;
        let mut command_buffer_builder = CommandBufferBuilder::new(&backend_shared.device, &mut command_buffer)?;
        command_buffer_builder.begin_command_buffer_for_one_time_submit()?;

        // Handle inanimate mesh events
        while let Some(event) = backend_shared.inanimate_mesh_event_queue.lock().pop() {
            match event {
                InanimateMeshEvent::Insert {
                    inanimate_mesh,
                    vertex_positions,
                    indices: _,
                } => {
                    let _span = span!("Insert inanimate mesh");

                    let vertex_positions4 = vertex_positions
                        .iter()
                        .map(|v| Vector4::new(v.x, v.y, v.z, 1.0))
                        .collect::<Vec<_>>();
                    let vertices_start_offset = backend_shared
                        .static_vertex_buffer
                        .lock()
                        .push(&vertex_positions4, &mut command_buffer_builder)?;
                    let inanimate_mesh_gpu = shader_interface::InanimateMesh {
                        start_offset: vertices_start_offset as u64,
                        vertices_len: vertex_positions.len() as u64,
                    };
                    info!(
                        "Inserting a new inanimate mesh with start_offset: {start_offset} and vertices_len: {vertices_len}",
                        start_offset = vertices_start_offset,
                        vertices_len = vertex_positions.len()
                    );
                    let inanimate_mesh_start_offset = backend_shared
                        .inanimate_mesh_buffer
                        .lock()
                        .push(&[inanimate_mesh_gpu], &mut command_buffer_builder)?;
                    let inanimate_mesh2 = inanimate_mesh.clone();
                    let inanimate_mesh_gpu_states2 = backend_shared.inanimate_mesh_gpu_states.clone();

                    // Insert the GPU state for the InanimateMesh when the upload to the GPU is done
                    command_buffer_builder.push_finished_operation(Box::new(move || {
                        let handle = inanimate_mesh2.handle();
                        inanimate_mesh_gpu_states2.lock().insert(
                            handle.clone(),
                            InanimateMeshGpuState::Uploaded {
                                inanimate_mesh_offset: inanimate_mesh_start_offset as u64,
                            },
                        );
                        info!(
                            "Upload of inanimate mesh {} ({:?}) to GPU is done",
                            inanimate_mesh.as_debug_info().format_one_line(),
                            handle
                        );
                        Ok(())
                    }));
                }
                InanimateMeshEvent::SetVertexPositions {
                    inanimate_mesh: _,
                    vertex_posisions: _,
                } => {
                    todo!()
                }
            }
        }
        command_buffer_builder.end_command_buffer()?;
        queue.submit(command_buffer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod backend_new {
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
            AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
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

    mod render_frame {
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
            let backend = AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
            backend.handle_render_frame().unwrap();
        }
    }
}
