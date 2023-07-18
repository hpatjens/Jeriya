use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use jeriya_backend_ash_base::{
    buffer::BufferUsageFlags,
    command_pool::{CommandPool, CommandPoolCreateFlags},
    device::Device,
    queue::{Queue, QueueType},
    shader_interface,
    staged_push_only_buffer::StagedPushOnlyBuffer,
};
use jeriya_shared::{
    debug_info,
    inanimate_mesh::{InanimateMeshEvent, InanimateMeshGpuState, InanimateMeshGroup},
    log::info,
    nalgebra::Vector4,
    parking_lot::Mutex,
    Camera, CameraEvent, EventQueue, Handle, InanimateMesh, InanimateMeshInstance, InanimateMeshInstanceEvent, IndexingContainer,
    RendererConfig,
};

/// Elements of the backend that are shared between all [`Presenter`]s.
pub struct BackendShared {
    pub device: Arc<Device>,
    pub renderer_config: Arc<RendererConfig>,
    pub presentation_queue: RefCell<Queue>,
    pub command_pool: Rc<CommandPool>,
    pub cameras: Arc<Mutex<IndexingContainer<Camera>>>,
    pub camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,
    pub inanimate_meshes: Arc<InanimateMeshGroup>,
    pub inanimate_mesh_gpu_states: Arc<Mutex<HashMap<Handle<Arc<InanimateMesh>>, InanimateMeshGpuState>>>,
    pub inanimate_mesh_event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
    pub inanimate_mesh_buffer: Mutex<StagedPushOnlyBuffer<shader_interface::InanimateMesh>>,
    pub static_vertex_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,
    pub inanimate_mesh_instances: Arc<Mutex<IndexingContainer<InanimateMeshInstance>>>,
    pub inanimate_mesh_instance_event_queue: Arc<Mutex<EventQueue<InanimateMeshInstanceEvent>>>,
}

impl BackendShared {
    pub fn new(device: &Arc<Device>, renderer_config: &Arc<RendererConfig>) -> jeriya_shared::Result<Self> {
        info!("Creating Cameras");
        let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
        let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        info!("Creating InanimateMeshes");
        let inanimate_mesh_event_queue = Arc::new(Mutex::new(EventQueue::new()));
        let inanimate_meshes = Arc::new(InanimateMeshGroup::new(inanimate_mesh_event_queue.clone()));

        info!("Creating InanimateMeshInstances");
        let inanimate_mesh_instances = Arc::new(Mutex::new(IndexingContainer::new()));
        let inanimate_mesh_instance_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        // Presentation Queue
        let presentation_queue = Queue::new(device, QueueType::Presentation)?;

        info!("Creating CommandPool");
        let command_pool = CommandPool::new(
            device,
            &presentation_queue,
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;

        info!("Creating StagedPushOnlyBuffer for InanimateMeshes");
        const INANIMATE_MESH_BUFFER_CAPACITY: usize = 100;
        let inanimate_mesh_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            INANIMATE_MESH_BUFFER_CAPACITY,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("InanimateMeshBuffer"),
        )?);

        info!("Creating static vertex buffer");
        const STATIC_VERTEX_BUFFER_CAPACITY: usize = 1_000_000;
        let static_vertex_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            STATIC_VERTEX_BUFFER_CAPACITY,
            BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("static_vertex_buffer"),
        )?);

        Ok(Self {
            device: device.clone(),
            renderer_config: renderer_config.clone(),
            presentation_queue: RefCell::new(presentation_queue),
            command_pool,
            cameras,
            camera_event_queue,
            inanimate_meshes,
            inanimate_mesh_event_queue,
            inanimate_mesh_buffer,
            static_vertex_buffer,
            inanimate_mesh_gpu_states: Arc::new(Mutex::new(HashMap::new())),
            inanimate_mesh_instances,
            inanimate_mesh_instance_event_queue,
        })
    }
}
