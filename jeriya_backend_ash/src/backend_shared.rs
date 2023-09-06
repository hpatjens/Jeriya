use std::{collections::HashMap, sync::Arc};

use jeriya_backend::{
    inanimate_mesh::{InanimateMeshEvent, InanimateMeshGpuState, InanimateMeshGroup},
    model::ModelGroup,
    Camera, CameraEvent, InanimateMesh, InanimateMeshInstance, InanimateMeshInstanceEvent, ModelInstance, ModelInstanceEvent,
};
use jeriya_backend_ash_base::{buffer::BufferUsageFlags, device::Device, shader_interface, staged_push_only_buffer::StagedPushOnlyBuffer};
use jeriya_shared::{debug_info, log::info, nalgebra::Vector4, parking_lot::Mutex, EventQueue, Handle, IndexingContainer, RendererConfig};

/// Elements of the backend that are shared between all [`Presenter`]s.
pub struct BackendShared {
    pub device: Arc<Device>,
    pub renderer_config: Arc<RendererConfig>,

    pub cameras: Arc<Mutex<IndexingContainer<Camera>>>,
    pub camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,

    pub inanimate_mesh_group: Arc<InanimateMeshGroup>,
    pub inanimate_mesh_gpu_states: Arc<Mutex<HashMap<Handle<Arc<InanimateMesh>>, InanimateMeshGpuState>>>,
    pub inanimate_mesh_event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
    pub inanimate_mesh_buffer: Mutex<StagedPushOnlyBuffer<shader_interface::InanimateMesh>>,

    pub model_group: Arc<ModelGroup>,

    pub static_vertex_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,

    pub inanimate_mesh_instances: Arc<Mutex<IndexingContainer<InanimateMeshInstance>>>,
    pub inanimate_mesh_instance_event_queue: Arc<Mutex<EventQueue<InanimateMeshInstanceEvent>>>,

    pub model_instances: Arc<Mutex<IndexingContainer<ModelInstance>>>,
    pub model_instance_event_queue: Arc<Mutex<EventQueue<ModelInstanceEvent>>>,
}

impl BackendShared {
    pub fn new(device: &Arc<Device>, renderer_config: &Arc<RendererConfig>) -> jeriya_backend::Result<Self> {
        info!("Creating Cameras");
        let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
        let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        info!("Creating InanimateMeshes");
        let inanimate_mesh_event_queue = Arc::new(Mutex::new(EventQueue::new()));
        let inanimate_meshes = Arc::new(InanimateMeshGroup::new(inanimate_mesh_event_queue.clone()));

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
            cameras,
            camera_event_queue,
            inanimate_mesh_group: inanimate_meshes,
            inanimate_mesh_event_queue,
            inanimate_mesh_buffer,
            model_group,
            static_vertex_buffer,
            inanimate_mesh_gpu_states: Arc::new(Mutex::new(HashMap::new())),
            inanimate_mesh_instances,
            inanimate_mesh_instance_event_queue,
            model_instances,
            model_instance_event_queue,
        })
    }
}
