use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc},
};

use jeriya_backend::{
    elements::{self, point_cloud::PointCloud, rigid_mesh::RigidMesh},
    gpu_index_allocator::GpuIndexAllocator,
    instances::{camera_instance::CameraInstance, rigid_mesh_instance::RigidMeshInstance},
    resources::{
        mesh_attributes::{MeshAttributes, MeshAttributesGpuState},
        point_cloud_attributes::{PointCloudAttributes, PointCloudAttributesGpuState},
        ResourceEvent,
    },
};
use jeriya_backend_ash_base::{
    buffer::BufferUsageFlags, device::Device, host_visible_buffer::HostVisibleBuffer, shader_interface,
    staged_push_only_buffer::StagedPushOnlyBuffer,
};
use jeriya_shared::{debug_info, log::info, nalgebra::Vector4, parking_lot::Mutex, Handle, RendererConfig};

use crate::queue_scheduler::QueueScheduler;

/// Elements of the backend that are shared between all [`Presenter`]s.
pub struct BackendShared {
    pub device: Arc<Device>,
    pub renderer_config: Arc<RendererConfig>,

    pub queue_scheduler: QueueScheduler,

    pub resource_event_sender: Sender<ResourceEvent>,

    pub mesh_attributes_gpu_states: Arc<Mutex<HashMap<Handle<Arc<MeshAttributes>>, MeshAttributesGpuState>>>,
    pub mesh_attributes_buffer: Mutex<HostVisibleBuffer<shader_interface::MeshAttributes>>,

    pub point_cloud_attributes_gpu_states: Arc<Mutex<HashMap<Handle<Arc<PointCloudAttributes>>, PointCloudAttributesGpuState>>>,
    pub point_cloud_attributes_buffer: Mutex<HostVisibleBuffer<shader_interface::PointCloudAttributes>>,

    pub static_vertex_position_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,
    pub static_vertex_normals_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,
    pub static_indices_buffer: Mutex<StagedPushOnlyBuffer<u32>>,
    pub static_meshlet_buffer: Mutex<StagedPushOnlyBuffer<shader_interface::Meshlet>>,
    pub static_point_positions_buffer: Mutex<StagedPushOnlyBuffer<Vector4<f32>>>,

    pub mesh_attributes_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<MeshAttributes>>>,
    pub point_cloud_attributes_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<PointCloudAttributes>>>,
    pub camera_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<elements::camera::Camera>>>,
    pub camera_instance_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<CameraInstance>>>,
    pub rigid_mesh_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<RigidMesh>>>,
    pub rigid_mesh_instance_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<RigidMeshInstance>>>,
    pub point_cloud_gpu_index_allocator: Arc<Mutex<GpuIndexAllocator<PointCloud>>>,
}

impl BackendShared {
    pub fn new(
        device: &Arc<Device>,
        renderer_config: &Arc<RendererConfig>,
        resource_sender: Sender<ResourceEvent>,
    ) -> jeriya_backend::Result<Self> {
        info!("Creating HostVisibleBuffer for MeshAttributes");
        let mesh_attributes_buffer = Mutex::new(HostVisibleBuffer::new(
            device,
            &vec![shader_interface::MeshAttributes::default(); renderer_config.maximum_number_of_mesh_attributes],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("mesh_attributes_buffer"),
        )?);

        info!("Creating HostVisibleBuffer for PointCloudAttributes");
        let point_cloud_attributes_buffer = Mutex::new(HostVisibleBuffer::new(
            device,
            &vec![shader_interface::PointCloudAttributes::default(); renderer_config.maximum_number_of_point_cloud_attributes],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("point_cloud_attribute_buffer"),
        )?);

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

        info!("Creating static point positions buffer");
        const STATIC_POINT_POSITIONS_BUFFER_CAPACITY: usize = 16_000_000;
        let static_point_positions_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            STATIC_POINT_POSITIONS_BUFFER_CAPACITY,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("static_point_positions_buffer"),
        )?);

        info!("Creating static meshlet buffer");
        let static_meshlet_buffer = Mutex::new(StagedPushOnlyBuffer::new(
            device,
            renderer_config.maximum_meshlets,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("static_meshlet_buffer"),
        )?);

        info!("Creating the QueueScheduler");
        let queue_scheduler = QueueScheduler::new(device)?;

        info!("Creating the GpuIndexAllocators");
        fn new_allocator<T>(max_count: usize) -> Arc<Mutex<GpuIndexAllocator<T>>> {
            Arc::new(Mutex::new(GpuIndexAllocator::new(max_count)))
        }
        let camera_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_cameras);
        let camera_instance_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_camera_instances);
        let rigid_mesh_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_rigid_meshes);
        let mesh_attributes_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_mesh_attributes);
        let point_cloud_attributes_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_point_cloud_attributes);
        let rigid_mesh_instance_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_rigid_mesh_instances);
        let point_cloud_gpu_index_allocator = new_allocator(renderer_config.maximum_number_of_point_clouds);

        Ok(Self {
            device: device.clone(),
            renderer_config: renderer_config.clone(),
            queue_scheduler,
            resource_event_sender: resource_sender,
            mesh_attributes_buffer,
            mesh_attributes_gpu_states: Arc::new(Mutex::new(HashMap::new())),
            point_cloud_attributes_buffer,
            point_cloud_attributes_gpu_states: Arc::new(Mutex::new(HashMap::new())),
            static_vertex_position_buffer,
            static_vertex_normals_buffer,
            static_indices_buffer,
            static_meshlet_buffer,
            static_point_positions_buffer,
            mesh_attributes_gpu_index_allocator,
            point_cloud_attributes_gpu_index_allocator,
            camera_gpu_index_allocator,
            camera_instance_gpu_index_allocator,
            rigid_mesh_gpu_index_allocator,
            rigid_mesh_instance_gpu_index_allocator,
            point_cloud_gpu_index_allocator,
        })
    }
}
