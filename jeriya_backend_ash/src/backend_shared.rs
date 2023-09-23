use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc},
};

use jeriya_backend::{
    inanimate_mesh::{InanimateMeshGpuState, InanimateMeshGroup},
    model::ModelGroup,
    Camera, CameraEvent, InanimateMesh, InanimateMeshInstance, InanimateMeshInstanceEvent, ModelInstance, ModelInstanceEvent,
    ResourceEvent,
};
use jeriya_backend_ash_base::{buffer::BufferUsageFlags, device::Device, shader_interface, staged_push_only_buffer::StagedPushOnlyBuffer};
use jeriya_shared::{debug_info, log::info, nalgebra::Vector4, parking_lot::Mutex, EventQueue, Handle, IndexingContainer, RendererConfig};

use crate::queue_scheduler::QueueScheduler;

/// Elements of the backend that are shared between all [`Presenter`]s.
pub struct BackendShared {
    pub device: Arc<Device>,
    pub renderer_config: Arc<RendererConfig>,

    pub queue_scheduler: QueueScheduler,

    pub resource_event_sender: Sender<ResourceEvent>,

    pub camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,
    pub cameras: Arc<Mutex<IndexingContainer<Camera>>>,

    pub inanimate_mesh_group: Arc<InanimateMeshGroup>,
    pub inanimate_mesh_gpu_states: Arc<Mutex<HashMap<Handle<Arc<InanimateMesh>>, InanimateMeshGpuState>>>,
    pub inanimate_mesh_buffer: Mutex<StagedPushOnlyBuffer<shader_interface::InanimateMesh>>,

    pub model_group: Arc<ModelGroup>,

    pub static_vertex_position_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,
    pub static_vertex_normals_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,
    pub static_indices_buffer: Mutex<StagedPushOnlyBuffer<u32>>,

    pub inanimate_mesh_instance_event_queue: Arc<Mutex<EventQueue<InanimateMeshInstanceEvent>>>,
    pub inanimate_mesh_instances: Arc<Mutex<IndexingContainer<InanimateMeshInstance>>>,

    pub model_instance_event_queue: Arc<Mutex<EventQueue<ModelInstanceEvent>>>,
    pub model_instances: Arc<Mutex<IndexingContainer<ModelInstance>>>,
}

impl BackendShared {
    pub fn new(
        device: &Arc<Device>,
        renderer_config: &Arc<RendererConfig>,
        resource_sender: Sender<ResourceEvent>,
    ) -> jeriya_backend::Result<Self> {
        info!("Creating Cameras");
        let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
        let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        info!("Creating InanimateMeshes");
        let inanimate_meshes = Arc::new(InanimateMeshGroup::new(resource_sender.clone()));

        info!("Creating InanimateMeshInstances");
        let inanimate_mesh_instances = Arc::new(Mutex::new(IndexingContainer::new()));
        let inanimate_mesh_instance_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        info!("Creating ModelInstances");
        let model_instances = Arc::new(Mutex::new(IndexingContainer::new()));
        let model_instance_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        info!("Creating StagedPushOnlyBuffer for InanimateMeshes");
        const INANIMATE_MESH_BUFFER_CAPACITY: usize = 100;
        let inanimate_mesh_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            INANIMATE_MESH_BUFFER_CAPACITY,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("InanimateMeshBuffer"),
        )?);

        info!("Creating ModelGroup");
        let model_group = Arc::new(ModelGroup::new(&inanimate_meshes));

        info!("Creating static vertex positions buffer");
        const STATIC_VERTEX_POSITION_BUFFER_CAPACITY: usize = 1_000_000;
        let static_vertex_position_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            STATIC_VERTEX_POSITION_BUFFER_CAPACITY,
            BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("static_vertex_positions_buffer"),
        )?);

        info!("Creating static vertex normals buffer");
        const STATIC_VERTEX_NORMALS_BUFFER_CAPACITY: usize = 1_000_000;
        let static_vertex_normals_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            STATIC_VERTEX_NORMALS_BUFFER_CAPACITY,
            BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("static_vertex_normals_buffer"),
        )?);

        info!("Creating static indices buffer");
        const STATIC_INDICES_BUFFER_CAPACITY: usize = 1_000_000;
        let static_indices_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            STATIC_INDICES_BUFFER_CAPACITY,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("static_indices_buffer"),
        )?);

        info!("Creating the QueueScheduler");
        let queue_scheduler = QueueScheduler::new(device)?;

        Ok(Self {
            device: device.clone(),
            renderer_config: renderer_config.clone(),
            queue_scheduler,
            resource_event_sender: resource_sender,
            cameras,
            camera_event_queue,
            inanimate_mesh_group: inanimate_meshes,
            inanimate_mesh_buffer,
            model_group,
            static_vertex_position_buffer,
            static_vertex_normals_buffer,
            static_indices_buffer,
            inanimate_mesh_gpu_states: Arc::new(Mutex::new(HashMap::new())),
            inanimate_mesh_instances,
            inanimate_mesh_instance_event_queue,
            model_instances,
            model_instance_event_queue,
        })
    }
}
