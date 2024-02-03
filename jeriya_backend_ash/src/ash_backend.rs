use std::{
    collections::HashMap,
    iter,
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
    elements::{self, point_cloud::PointCloud, rigid_mesh::RigidMesh},
    gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation},
    immediate::{self, ImmediateRenderingFrame},
    instances::{camera_instance::CameraInstance, point_cloud_instance::PointCloudInstance, rigid_mesh_instance::RigidMeshInstance},
    resources::{
        mesh_attributes::{MeshAttributes, MeshAttributesGpuState},
        mesh_attributes_group::MeshAttributesEvent,
        point_cloud_attributes::{PointCloudAttributes, PointCloudAttributesGpuState},
        point_cloud_attributes_group::PointCloudAttributesEvent,
        ResourceEvent, ResourceReceiver,
    },
    transactions::{self, PushEvent, Transaction, TransactionProcessor},
    Backend, ImmediateCommandBufferBuilderHandler,
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
use jeriya_content::{
    asset_importer::{AssetImporter, FileSystem},
    model::Meshlet,
    point_cloud::clustered_point_cloud::Page,
};
use jeriya_macros::profile;
use jeriya_shared::{
    debug_info,
    log::{error, info, trace},
    nalgebra::Vector4,
    tracy_client::Client,
    winit::window::{Window, WindowId},
    AsDebugInfo, DebugInfo, FrameRate, RendererConfig, WindowConfig,
};
use jeriya_test::create_window;

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

impl AllocateGpuIndex<elements::camera::Camera> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<elements::camera::Camera>> {
        self.backend_shared.camera_gpu_index_allocator.lock().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<elements::camera::Camera>) {
        self.backend_shared
            .camera_gpu_index_allocator
            .lock()
            .free_gpu_index(gpu_index_allocation);
    }
}

impl AllocateGpuIndex<CameraInstance> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<CameraInstance>> {
        self.backend_shared.camera_instance_gpu_index_allocator.lock().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<CameraInstance>) {
        self.backend_shared
            .camera_instance_gpu_index_allocator
            .lock()
            .free_gpu_index(gpu_index_allocation);
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

impl AllocateGpuIndex<PointCloud> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<PointCloud>> {
        self.backend_shared.point_cloud_gpu_index_allocator.lock().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<PointCloud>) {
        self.backend_shared
            .point_cloud_gpu_index_allocator
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

impl AllocateGpuIndex<PointCloudInstance> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<PointCloudInstance>> {
        self.backend_shared
            .point_cloud_instance_gpu_index_allocator
            .lock()
            .allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<PointCloudInstance>) {
        self.backend_shared
            .point_cloud_instance_gpu_index_allocator
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

impl AllocateGpuIndex<PointCloudAttributes> for AshBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<PointCloudAttributes>> {
        self.backend_shared
            .point_cloud_attributes_gpu_index_allocator
            .lock()
            .allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<PointCloudAttributes>) {
        self.backend_shared
            .point_cloud_attributes_gpu_index_allocator
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
        asset_importer: Arc<AssetImporter>,
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

        let backend_shared = Arc::new(BackendShared::new(
            &device,
            &Arc::new(renderer_config),
            resource_event_sender,
            &asset_importer,
        )?);

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

    fn set_active_camera(&self, window_id: WindowId, camera_instance: &CameraInstance) -> jeriya_backend::Result<()> {
        let presenter = self
            .presenters
            .get(&window_id)
            .ok_or(jeriya_backend::Error::UnknownWindowId(window_id))?;
        presenter.set_active_camera(camera_instance);
        Ok(())
    }
}

fn run_resource_thread(resource_event_receiver: Receiver<ResourceEvent>, backend: &Arc<AshBackend>) -> jeriya_backend::Result<()> {
    loop {
        let Ok(resource_event) = resource_event_receiver.recv() else {
            panic!("failed to receive frame start");
        };

        let backend_shared = &backend.backend_shared;

        let queue_poll_span = jeriya_shared::span!("Poll queues");
        let mut queues = backend_shared.queue_scheduler.queues();
        queues.transfer_queue().poll_completed_fences()?;
        drop(queues);
        drop(queue_poll_span);

        match resource_event {
            ResourceEvent::FrameStart => {}
            ResourceEvent::MeshAttributes(mesh_attributes_events) => {
                handle_mesh_attributes_events(backend, mesh_attributes_events)?;
            }
            ResourceEvent::PointCloudAttributes(point_cloud_attributes_events) => {
                handle_point_cloud_attributes_events(backend, point_cloud_attributes_events)?;
            }
        }
    }
}

#[profile]
fn handle_point_cloud_attributes_events(
    backend: &Arc<AshBackend>,
    point_cloud_attributes_events: Vec<PointCloudAttributesEvent>,
) -> jeriya_backend::Result<()> {
    let backend_shared = &backend.backend_shared;

    info!("Creating CommandPool");
    let mut queues = backend_shared.queue_scheduler.queues();
    let command_pool = CommandPool::new(
        &backend_shared.device,
        queues.transfer_queue(),
        CommandPoolCreateFlags::ResetCommandBuffer,
        debug_info!("PointCloudAttributes-CommandPool"),
    )?;
    drop(queues);

    // Create a new command buffer for maintaining the meshes
    let mut command_buffer = CommandBuffer::new(
        &backend_shared.device,
        &command_pool.clone(),
        debug_info!("PointCloudAttributes-CommandBuffer"),
    )?;
    let mut command_buffer_builder = CommandBufferBuilder::new(&backend_shared.device, &mut command_buffer)?;
    command_buffer_builder.begin_command_buffer_for_one_time_submit()?;

    // Handle mesh attributes events
    for point_cloud_attributes_event in point_cloud_attributes_events {
        match point_cloud_attributes_event {
            PointCloudAttributesEvent::Insert {
                handle,
                point_cloud_attributes,
            } => {
                let _span = jeriya_shared::span!("Insert point cloud attributes");

                // Upload the point positions to the GPU
                let point_positions4 = point_cloud_attributes
                    .point_positions()
                    .iter()
                    .map(|v| Vector4::new(v.x, v.y, v.z, 1.0))
                    .collect::<Vec<_>>();
                let point_positions_start_offset = backend_shared
                    .static_point_positions_buffer
                    .lock()
                    .push(&point_positions4, &mut command_buffer_builder)?
                    .unwrap_or(0);

                // Upload the point colors to the GPU
                let point_colors4 = point_cloud_attributes
                    .point_colors()
                    .iter()
                    .map(|v| v.as_vector4())
                    .collect::<Vec<_>>();
                let point_colors_start_offset = backend_shared
                    .static_point_colors_buffer
                    .lock()
                    .push(&point_colors4, &mut command_buffer_builder)?
                    .unwrap_or(0);

                // Upload the pages to the GPU
                let point_cloud_pages = point_cloud_attributes
                    .pages()
                    .iter()
                    .map(|page| {
                        let point_positions = page
                            .point_positions()
                            .iter()
                            .map(|v| Vector4::new(v.x, v.y, v.z, 0.0))
                            .chain(std::iter::repeat(Vector4::zeros()).take(Page::MAX_POINTS - page.point_positions().len()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .expect("point positions have wrong length");
                        let point_colors = page
                            .point_colors()
                            .iter()
                            .map(|v| v.as_vector4())
                            .chain(std::iter::repeat(Vector4::zeros()).take(Page::MAX_POINTS - page.point_colors().len()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .expect("point colors have wrong length");
                        let padding = std::iter::repeat(shader_interface::PointCloudCluster::default())
                            .take(Page::MAX_CLUSTERS - page.clusters().len());
                        let clusters = page
                            .clusters()
                            .iter()
                            .map(|cluster| shader_interface::PointCloudCluster {
                                center_radius: Vector4::new(cluster.center.x, cluster.center.y, cluster.center.z, cluster.radius),
                                points_start_offset: cluster.index_start,
                                points_len: cluster.len,
                                level: cluster.level as u32,
                                depth: cluster.depth as u32,
                                children_count: cluster.children.len() as u32,
                                children_page_indices: cluster
                                    .children
                                    .iter()
                                    .map(|child| child.page_index as u32)
                                    .chain(std::iter::repeat(u32::MAX).take(2 - cluster.children.len()))
                                    .collect::<Vec<_>>()
                                    .try_into()
                                    .expect("clusters have wrong length"),
                                children_cluster_indices: cluster
                                    .children
                                    .iter()
                                    .map(|child| child.cluster_index as u32)
                                    .chain(std::iter::repeat(u32::MAX).take(2 - cluster.children.len()))
                                    .collect::<Vec<_>>()
                                    .try_into()
                                    .expect("clusters have wrong length"),
                                padding: [0; 3],
                            })
                            .chain(padding)
                            .collect::<Vec<_>>()
                            .try_into()
                            .expect("clusters have wrong length");
                        shader_interface::PointCloudPage {
                            points_len: page.point_positions().len() as u32,
                            clusters_len: page.clusters().len() as u32,
                            _padding: [0; 2],
                            point_positions,
                            point_colors,
                            clusters,
                        }
                    })
                    .collect::<Vec<_>>();

                let pages_start_offset = backend_shared
                    .static_point_cloud_pages_buffer
                    .lock()
                    .push(&point_cloud_pages, &mut command_buffer_builder)?
                    .unwrap_or(0);

                // Upload the PointCloudAttributes to the GPU
                let points_len = point_cloud_attributes.point_positions().len() as u32;
                let point_positions_start_offset = point_positions_start_offset as u32;
                let point_colors_start_offset = point_colors_start_offset as u32;
                let pages_len = point_cloud_attributes.pages().len() as u32;
                let pages_start_offset = pages_start_offset as u32;

                let point_cloud_attributes_gpu = shader_interface::PointCloudAttributes {
                    points_len,
                    point_positions_start_offset,
                    point_colors_start_offset,
                    pages_len,
                    pages_start_offset,
                    root_cluster_page_index: point_cloud_attributes.root_cluster_index().page_index as u32,
                    root_cluster_cluster_index: point_cloud_attributes.root_cluster_index().cluster_index as u32,
                };
                trace!("Root cluster page index: {}", point_cloud_attributes_gpu.root_cluster_page_index);
                trace!(
                    "Root cluster cluster index: {}",
                    point_cloud_attributes_gpu.root_cluster_cluster_index
                );

                info!("Inserting a new PointCloudAttributes: {point_cloud_attributes_gpu:#?}",);
                backend_shared
                    .point_cloud_attributes_buffer
                    .lock()
                    .set_memory_unaligned_index(point_cloud_attributes.gpu_index_allocation().index(), &point_cloud_attributes_gpu)?;

                // Insert the GPU state for the PointCloudAttributes when the upload to the GPU is done
                let point_cloud_attributes_gpu_states2 = backend_shared.point_cloud_attributes_gpu_states.clone();
                let backend2 = backend.clone();
                command_buffer_builder.push_finished_operation(Box::new(move || {
                    point_cloud_attributes_gpu_states2
                        .lock()
                        .insert(handle, PointCloudAttributesGpuState::Uploaded);

                    // Notify the frames that the PointCloudAttributes are ready
                    let mut transaction = Transaction::new();
                    transaction.push_event(transactions::Event::SetPointCloudAttributesActive {
                        gpu_index_allocation: *point_cloud_attributes.gpu_index_allocation(),
                        is_active: true,
                    });
                    backend2.process(transaction);

                    info! {
                        "Upload of PointCloudAttributes {} ({:?}) to GPU is done",
                        point_cloud_attributes.debug_info().format_one_line(),
                        handle
                    }
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

#[profile]
fn handle_mesh_attributes_events(
    backend: &Arc<AshBackend>,
    mesh_attributes_events: Vec<MeshAttributesEvent>,
) -> jeriya_backend::Result<()> {
    let _span = jeriya_shared::span!("Handle mesh attributes events");

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
                let _span = jeriya_shared::span!("Insert mesh attributes");

                // Upload the vertex positions to the GPU
                let vertex_positions4 = mesh_attributes
                    .vertex_positions()
                    .iter()
                    .map(|v| Vector4::new(v.x, v.y, v.z, 1.0))
                    .collect::<Vec<_>>();
                let vertex_positions_start_offset = backend_shared
                    .static_vertex_position_buffer
                    .lock()
                    .push(&vertex_positions4, &mut command_buffer_builder)?
                    .unwrap_or(0);

                // Upload the vertex normals to the GPU
                let vertex_normals4 = mesh_attributes
                    .vertex_normals()
                    .iter()
                    .map(|v| Vector4::new(v.x, v.y, v.z, 1.0))
                    .collect::<Vec<_>>();
                let vertex_normals_start_offset = backend_shared
                    .static_vertex_normals_buffer
                    .lock()
                    .push(&vertex_normals4, &mut command_buffer_builder)?
                    .unwrap_or(0);

                // Upload the indices to the GPU
                let indices_start_offset = if let Some(indices) = &mesh_attributes.indices() {
                    backend_shared
                        .static_indices_buffer
                        .lock()
                        .push(indices, &mut command_buffer_builder)?
                        .unwrap_or(0)
                } else {
                    0
                };

                // Upload the meshlets to the GPU
                let meshlets_start_offset = if let Some(meshlets) = &mesh_attributes.meshlets() {
                    let mut static_meshlet_buffer = backend_shared.static_meshlet_buffer.lock();
                    let meshlets = meshlets
                        .iter()
                        .map(|meshlet| {
                            assert! {
                                meshlet.global_indices.len() <= Meshlet::MAX_VERTICES,
                                "Meshlet references too many vertices. The validation of the MeshAttributes should have caught this."
                            }
                            // Pad the global indices with 0s
                            let global_indices = meshlet
                                .global_indices
                                .iter()
                                .cloned()
                                .chain(iter::repeat(0u32).take(Meshlet::MAX_VERTICES - meshlet.global_indices.len()))
                                .collect::<Vec<_>>()
                                .try_into()
                                .expect("Meshlet global indices are not 64 elements long");

                            // Pad the local indices with 0s
                            let local_indices = meshlet
                                .local_indices
                                .iter()
                                .map(|triangle| [triangle[0] as u32, triangle[1] as u32, triangle[2] as u32])
                                .chain(iter::repeat([0; 3]).take(Meshlet::MAX_TRIANGLES - meshlet.local_indices.len()))
                                .collect::<Vec<_>>()
                                .try_into()
                                .expect("Meshlet local indices are not 126 elements long");

                            shader_interface::Meshlet {
                                global_indices,
                                local_indices,
                                vertex_count: meshlet.global_indices.len() as u32,
                                triangle_count: meshlet.local_indices.len() as u32,
                            }
                        })
                        .collect::<Vec<_>>();
                    static_meshlet_buffer.push(&meshlets, &mut command_buffer_builder)?.unwrap_or(0)
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
                let meshlets_start_offset = meshlets_start_offset as u64;
                let meshlets_len = mesh_attributes.meshlets().map(|meshlets| meshlets.len() as u64).unwrap_or(0);
                let mesh_attributes_gpu = shader_interface::MeshAttributes {
                    vertex_positions_start_offset,
                    vertex_positions_len,
                    indices_start_offset,
                    indices_len,
                    vertex_normals_start_offset,
                    vertex_normals_len,
                    meshlets_start_offset,
                    meshlets_len,
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
                    mesh_attributes_gpu_states2.lock().insert(handle, MeshAttributesGpuState::Uploaded);

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
            let asset_importer = Arc::new(AssetImporter::default_from("../assets/unprocessed").unwrap());
            AshBackend::new(renderer_config, backend_config, asset_importer, &[window_config]).unwrap();
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
            let asset_importer = Arc::new(AssetImporter::default_from("../assets/unprocessed").unwrap());
            AshBackend::new(renderer_config, backend_config, asset_importer, &[window_config]).unwrap();
        }

        #[test]
        fn empty_windows_none() {
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            let asset_importer = Arc::new(AssetImporter::default_from("../assets/unprocessed").unwrap());
            let result = AshBackend::new(renderer_config, backend_config, asset_importer, &[]);
            assert!(matches!(result, Err(jeriya_backend::Error::ExpectedWindow)));
        }
    }
}
