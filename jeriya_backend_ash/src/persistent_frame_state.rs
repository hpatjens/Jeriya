use std::{collections::VecDeque, mem, sync::Arc};

use base::{frame_local_buffer::FrameLocalBuffer, push_descriptors::PushDescriptors};
use jeriya_backend::{
    elements::{camera, point_cloud, rigid_mesh},
    instances::{camera_instance, point_cloud_instance, rigid_mesh_instance},
    transactions::{self, Transaction},
};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    buffer::BufferUsageFlags, command_buffer_builder::CommandBufferBuilder, command_buffer_builder::PipelineBindPoint,
    descriptor_set_layout::DescriptorSetLayout, device_visible_buffer::DeviceVisibleBuffer, host_visible_buffer::HostVisibleBuffer,
    semaphore::Semaphore, shader_interface, DispatchIndirectCommand, DrawIndirectCommand,
};
use jeriya_macros::profile;
use jeriya_shared::{debug_info, log::info, winit::window::WindowId};

use crate::backend_shared::BackendShared;

pub struct PersistentFrameState {
    pub presenter_index: usize,
    pub image_available_semaphore: Option<Arc<Semaphore>>,
    pub rendering_complete_semaphore: Option<Arc<Semaphore>>,

    pub per_frame_data_buffer: HostVisibleBuffer<shader_interface::PerFrameData>,
    pub frame_telemetry_buffer: HostVisibleBuffer<shader_interface::FrameTelemetry>,

    pub mesh_attributes_active_buffer: FrameLocalBuffer<u32>, // every u32 represents a bool
    pub point_cloud_attributes_active_buffer: FrameLocalBuffer<u32>, // every u32 represents a bool
    pub point_cloud_pages_active_buffer: FrameLocalBuffer<u32>, // every u32 represents a bool

    pub camera_buffer: FrameLocalBuffer<shader_interface::Camera>,
    pub camera_instance_buffer: FrameLocalBuffer<shader_interface::CameraInstance>,
    pub rigid_mesh_buffer: FrameLocalBuffer<shader_interface::RigidMesh>,
    pub rigid_mesh_instance_buffer: FrameLocalBuffer<shader_interface::RigidMeshInstance>,
    pub point_cloud_buffer: FrameLocalBuffer<shader_interface::PointCloud>,
    pub point_cloud_instance_buffer: FrameLocalBuffer<shader_interface::PointCloudInstance>,

    /// Contains the VkIndirectDrawCommands for the visible rigid mesh instances that will
    /// be rendered with the simple mesh representation and not with meshlets.
    pub visible_rigid_mesh_instances_simple_buffer: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible rigid mesh instances.
    /// At the front of the buffer is a counter that contains the number of visible instances.
    pub visible_rigid_mesh_instances: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible meshlets of the visible rigid mesh instances.
    /// At the front of the buffer is a counter that contains the number of visible meshlets.
    pub visible_rigid_mesh_meshlets: Arc<DeviceVisibleBuffer<u32>>,

    /// Contains the indices of the visible point cloud instances as well as the indirect
    /// rendering commands preceded with a counter that contains the number of visible instances.
    pub visible_point_cloud_instances_simple: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible point cloud instances that will be rendered with
    /// the clusters. This buffer contains the count and the indices of the visible point clouds.
    pub visible_point_cloud_instances: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible point cloud clusters. This buffer contains the count
    /// and the indices of the visible point cloud clusters.
    pub visible_point_cloud_clusters: Arc<DeviceVisibleBuffer<u32>>,

    /// Buffer to which lines for debugging are written.
    /// Layout: [line1_start, line1_end, line1_color, line2_start, ...]
    pub device_local_debug_lines_buffer: Arc<DeviceVisibleBuffer<f32>>,

    pub transactions: VecDeque<Transaction>,
}

#[profile]
impl PersistentFrameState {
    pub fn new(presenter_index: usize, window_id: &WindowId, backend_shared: &BackendShared) -> base::Result<Self> {
        let per_frame_data_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &[shader_interface::PerFrameData::default(); 1],
            BufferUsageFlags::UNIFORM_BUFFER,
            debug_info!(format!("PerFrameDataBuffer-for-Window{:?}", window_id)),
        )?;

        let frame_telemetry_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &[shader_interface::FrameTelemetry::default(); 1],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!(format!("FrameTelemetryBuffer-for-Window{:?}", window_id)),
        )?;

        // Create camera buffer
        let len = backend_shared.renderer_config.maximum_number_of_cameras;
        info!("Create camera buffer with length: {len}");
        let camera_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("CameraBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_camera_instances;
        info!("Create camera instance buffer with length: {len}");
        let camera_instance_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("CameraInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_mesh_attributes;
        info!("Create mesh attributes active buffer with length: {len}");
        let mesh_attributes_active_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("MeshAttributesActiveBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_cloud_attributes;
        info!("Create point cloud attributes active buffer with length: {len}");
        let point_cloud_attributes_active_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudAttributesActiveBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_cloud_pages;
        info!("Create point cloud pages active buffer with length: {len}");
        let point_cloud_pages_active_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudPagesActiveBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_rigid_meshes;
        info!("Create rigid mesh buffer with length: {len}");
        let rigid_mesh_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("RigidMeshBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_rigid_mesh_instances;
        info!("Create rigid mesh instance buffer with length: {len}");
        let rigid_mesh_instance_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("RigidMeshInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_clouds;
        info!("Create point cloud buffer with length: {len}");
        let point_cloud_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_cloud_instances;
        info!("Create point cloud instance buffer with length: {len}");
        let point_cloud_instance_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create indirect draw buffer");
        let byte_size_draw_indirect_commands =
            backend_shared.renderer_config.maximum_number_of_rigid_mesh_instances * mem::size_of::<DrawIndirectCommand>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_rigid_mesh_instance_indices =
            backend_shared.renderer_config.maximum_number_of_rigid_mesh_instances * mem::size_of::<u32>();
        let visible_rigid_mesh_instances_simple_buffer = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_count + byte_size_draw_indirect_commands + byte_size_rigid_mesh_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("IndirectDrawBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible rigid mesh instances buffer");
        let byte_size_dispatch_indirect_command = mem::size_of::<DispatchIndirectCommand>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_indices = backend_shared.renderer_config.maximum_visible_rigid_mesh_instances * mem::size_of::<u32>();
        let visible_rigid_mesh_instances = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_dispatch_indirect_command + byte_size_count + byte_size_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisibleRigidMeshInstancesBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible rigid mesh meshlets buffer");
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_draw_indirect_commands =
            backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * mem::size_of::<DrawIndirectCommand>();
        let byte_size_meshlet_indices = backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * mem::size_of::<u32>();
        let byte_size_rigid_mesh_instance_indices =
            backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * mem::size_of::<u32>();
        let visible_rigid_mesh_meshlets = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_count + byte_size_meshlet_indices + byte_size_draw_indirect_commands + byte_size_rigid_mesh_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisibleRigidMeshMeshletsBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible point cloud instances buffer");
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_draw_indirect_commands =
            backend_shared.renderer_config.maximum_number_of_point_cloud_instances * mem::size_of::<DrawIndirectCommand>();
        let byte_size_point_cloud_instance_indices =
            backend_shared.renderer_config.maximum_number_of_point_cloud_instances * mem::size_of::<u32>();
        let visible_point_cloud_instances_simple = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_count + byte_size_draw_indirect_commands + byte_size_point_cloud_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisiblePointCloudInstancesBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible point cloud instances buffer");
        let byte_size_dispatch_indirect_command = mem::size_of::<DispatchIndirectCommand>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_point_cloud_instance_indices =
            backend_shared.renderer_config.maximum_number_of_point_cloud_instances * mem::size_of::<u32>();
        let visible_point_cloud_instances = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_dispatch_indirect_command + byte_size_count + byte_size_point_cloud_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisiblePointCloudInstancesBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible point cloud clusters buffer");
        let byte_size_dispatch_indirect_command =
            backend_shared.renderer_config.maximum_number_of_visible_point_cloud_clusters * mem::size_of::<DrawIndirectCommand>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_point_cloud_cluster_indices = backend_shared.renderer_config.maximum_number_of_visible_point_cloud_clusters
            * mem::size_of::<shader_interface::PointCloudClusterId>();
        let visible_point_cloud_clusters = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_dispatch_indirect_command + byte_size_count + byte_size_point_cloud_cluster_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisiblePointCloudClustersBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create device local debug lines buffer");
        let component_count = 3 + 3 + 4; // start, end, color
        let byte_size_line = component_count * mem::size_of::<f32>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_draw_indirect_command = mem::size_of::<DrawIndirectCommand>();
        let byte_size_debug_lines = backend_shared.renderer_config.maximum_number_of_device_local_debug_lines * byte_size_line;
        let byte_size = byte_size_draw_indirect_command + byte_size_count + byte_size_debug_lines;
        let device_local_debug_lines_buffer = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size,
            BufferUsageFlags::STORAGE_BUFFER | BufferUsageFlags::TRANSFER_DST_BIT | BufferUsageFlags::INDIRECT_BUFFER,
            debug_info!(format!("DeviceLocalDebugLinesBuffer-for-Window{:?}", window_id)),
        )?;

        Ok(Self {
            presenter_index,
            image_available_semaphore: None,
            rendering_complete_semaphore: None,
            per_frame_data_buffer,
            frame_telemetry_buffer,
            mesh_attributes_active_buffer,
            point_cloud_attributes_active_buffer,
            point_cloud_pages_active_buffer,
            camera_buffer,
            camera_instance_buffer,
            rigid_mesh_buffer,
            rigid_mesh_instance_buffer,
            point_cloud_buffer,
            point_cloud_instance_buffer,
            visible_rigid_mesh_instances_simple_buffer,
            visible_rigid_mesh_instances,
            visible_rigid_mesh_meshlets,
            visible_point_cloud_instances_simple,
            visible_point_cloud_instances,
            visible_point_cloud_clusters,
            device_local_debug_lines_buffer,
            transactions: VecDeque::new(),
        })
    }

    /// Pushes a [`Transaction`] to the frame to be processed when the frame is rendered.
    pub fn push_transaction(&mut self, transaction: Transaction) {
        self.transactions.push_back(transaction);
    }

    /// Sets the image available semaphore for the frame.
    pub fn set_image_available_semaphore(&mut self, image_available_semaphore: Arc<Semaphore>) {
        self.image_available_semaphore = Some(image_available_semaphore);
    }

    /// Returns the rendering complete semaphores for the frame.
    pub fn rendering_complete_semaphore(&self) -> Option<&Arc<Semaphore>> {
        self.rendering_complete_semaphore.as_ref()
    }

    /// Processes the [`Transaction`]s pushed to the frame.
    pub fn process_transactions(&mut self) -> base::Result<()> {
        use transactions::Event;
        let drain = self.transactions.drain(..).collect::<Vec<_>>();
        for transaction in drain {
            for event in transaction.process() {
                match event {
                    Event::RigidMesh(rigid_mesh) => self.process_rigid_mesh_event(rigid_mesh)?,
                    Event::RigidMeshInstance(rigid_mesh_instance_event) => {
                        self.process_rigid_mesh_instance_event(rigid_mesh_instance_event)?
                    }
                    Event::PointCloud(point_cloud) => self.process_point_cloud_event(point_cloud)?,
                    Event::PointCloudInstance(point_cloud_instance_event) => {
                        self.process_point_cloud_instance_event(point_cloud_instance_event)?
                    }
                    Event::Camera(camera_event) => self.process_camera_event(camera_event)?,
                    Event::CameraInstance(camera_instance_event) => self.process_camera_instance_event(camera_instance_event)?,
                    Event::SetMeshAttributeActive {
                        gpu_index_allocation,
                        is_active,
                    } => {
                        self.mesh_attributes_active_buffer
                            .set(&gpu_index_allocation, &if is_active { 1 } else { 0 })?;
                    }
                    Event::SetPointCloudAttributesActive {
                        gpu_index_allocation,
                        is_active,
                    } => self
                        .point_cloud_attributes_active_buffer
                        .set(&gpu_index_allocation, &if is_active { 1 } else { 0 })?,
                }
            }
        }
        Ok(())
    }

    /// Processes a [`rigid_mesh::Event`].
    fn process_rigid_mesh_event(&mut self, event: rigid_mesh::Event) -> base::Result<()> {
        use rigid_mesh::Event;
        match event {
            Event::Insert(rigid_mesh) => {
                self.rigid_mesh_buffer.set(
                    rigid_mesh.gpu_index_allocation(),
                    &shader_interface::RigidMesh {
                        mesh_attributes_index: rigid_mesh.mesh_attributes().gpu_index_allocation().index() as i32,
                        preferred_mesh_representation: (*rigid_mesh.preferred_mesh_representation()).into(),
                    },
                )?;
            }
            Event::Noop => {}
        }
        Ok(())
    }

    fn process_point_cloud_event(&mut self, event: point_cloud::Event) -> base::Result<()> {
        use point_cloud::Event;
        match event {
            Event::Insert(point_cloud) => {
                self.point_cloud_buffer.set(
                    point_cloud.gpu_index_allocation(),
                    &shader_interface::PointCloud {
                        point_cloud_attributes_index: point_cloud.point_cloud_attributes().gpu_index_allocation().index() as i32,
                        preferred_point_cloud_representation: (*point_cloud.preferred_point_cloud_representation()).into(),
                    },
                )?;
            }
            Event::Noop => {}
        }
        Ok(())
    }

    /// Processes a [`rigid_mesh_instance::Event`].
    fn process_rigid_mesh_instance_event(&mut self, event: rigid_mesh_instance::Event) -> base::Result<()> {
        use rigid_mesh_instance::Event;
        match event {
            Event::Noop => {}
            Event::Insert(rigid_mesh_instance) => {
                self.rigid_mesh_instance_buffer.set(
                    rigid_mesh_instance.gpu_index_allocation(),
                    &shader_interface::RigidMeshInstance {
                        rigid_mesh_index: rigid_mesh_instance.rigid_mesh_gpu_index_allocation().index() as u64,
                        _padding: 0,
                        transform: *rigid_mesh_instance.transform(),
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Processes a [`point_cloud_instance::Event`].
    fn process_point_cloud_instance_event(&mut self, event: point_cloud_instance::Event) -> base::Result<()> {
        use point_cloud_instance::Event;
        match event {
            Event::Noop => {}
            Event::Insert(point_cloud_instance) => {
                self.point_cloud_instance_buffer.set(
                    point_cloud_instance.gpu_index_allocation(),
                    &shader_interface::PointCloudInstance {
                        point_cloud_index: point_cloud_instance.point_cloud_gpu_index_allocation().index() as u64,
                        _padding: 0,
                        transform: *point_cloud_instance.transform(),
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Processes a [`camera::Event`].
    fn process_camera_event(&mut self, event: camera::Event) -> base::Result<()> {
        use camera::Event;
        match event {
            Event::Noop => {}
            Event::Insert(camera) => {
                info!("Insert Camera at {:?}", camera.gpu_index_allocation().index());
                self.camera_buffer.set(
                    camera.gpu_index_allocation(),
                    &shader_interface::Camera {
                        projection_matrix: camera.projection().projection_matrix(),
                        znear: camera.projection().znear(),
                        zfar: camera.projection().zfar(),
                        _padding: [0.0; 14],
                    },
                )?;
            }
            Event::UpdateProjection(gpu_index_allocation, projection) => {
                self.camera_buffer.set(
                    &gpu_index_allocation,
                    &shader_interface::Camera {
                        projection_matrix: projection.projection_matrix(),
                        znear: projection.znear(),
                        zfar: projection.zfar(),
                        _padding: [0.0; 14],
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Processes a [`camera_instance::Event`].
    fn process_camera_instance_event(&mut self, event: camera_instance::Event) -> base::Result<()> {
        use camera_instance::Event;
        match event {
            Event::Noop => {}
            Event::Insert(camera_instance) => {
                info!("Insert CameraInstance at {:?}", camera_instance.gpu_index_allocation().index());
                self.camera_instance_buffer.set(
                    camera_instance.gpu_index_allocation(),
                    &shader_interface::CameraInstance {
                        camera_index: camera_instance.camera_gpu_index_allocation().index() as u64,
                        _padding: 0,
                        view_matrix: camera_instance.transform().view_matrix(),
                    },
                )?;
            }
            Event::UpdateViewMatrix(gpu_index_allocation, matrix) => {
                self.camera_instance_buffer.set(
                    &gpu_index_allocation,
                    &shader_interface::CameraInstance {
                        camera_index: gpu_index_allocation.index() as u64,
                        _padding: 0,
                        view_matrix: matrix,
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Pushes the required descriptors to the [`CommandBufferBuilder`].
    pub fn push_descriptors(
        &self,
        pipeline_bind_point: PipelineBindPoint,
        descriptor_set_layout: &DescriptorSetLayout,
        backend_shared: &BackendShared,
        command_buffer_builder: &mut CommandBufferBuilder,
    ) -> base::Result<()> {
        let push_descriptors = &PushDescriptors::builder(descriptor_set_layout)
            .push_uniform_buffer(0, &self.per_frame_data_buffer)
            .push_storage_buffer(1, &self.camera_buffer)
            .push_storage_buffer(2, &self.camera_instance_buffer)
            .push_storage_buffer(3, &self.visible_rigid_mesh_instances_simple_buffer)
            .push_storage_buffer(5, &*backend_shared.static_vertex_position_buffer.lock())
            .push_storage_buffer(6, &*backend_shared.static_indices_buffer.lock())
            .push_storage_buffer(7, &*backend_shared.static_vertex_normals_buffer.lock())
            .push_storage_buffer(8, &*backend_shared.mesh_attributes_buffer.lock())
            .push_storage_buffer(9, &self.rigid_mesh_buffer)
            .push_storage_buffer(10, &self.mesh_attributes_active_buffer)
            .push_storage_buffer(11, &self.rigid_mesh_instance_buffer)
            .push_storage_buffer(12, &*backend_shared.static_meshlet_buffer.lock())
            .push_storage_buffer(13, &self.visible_rigid_mesh_instances)
            .push_storage_buffer(14, &self.visible_rigid_mesh_meshlets)
            .push_storage_buffer(15, &self.point_cloud_attributes_active_buffer)
            .push_storage_buffer(16, &self.point_cloud_buffer)
            .push_storage_buffer(17, &self.point_cloud_instance_buffer)
            .push_storage_buffer(18, &self.visible_point_cloud_instances_simple)
            .push_storage_buffer(19, &*backend_shared.point_cloud_attributes_buffer.lock())
            .push_storage_buffer(20, &*backend_shared.static_point_positions_buffer.lock())
            .push_storage_buffer(21, &*backend_shared.static_point_colors_buffer.lock())
            .push_storage_buffer(22, &*backend_shared.point_cloud_page_buffer.lock())
            .push_storage_buffer(23, &self.point_cloud_pages_active_buffer)
            .push_storage_buffer(24, &*backend_shared.static_point_cloud_pages_buffer.lock())
            .push_storage_buffer(25, &self.visible_point_cloud_instances)
            .push_storage_buffer(26, &self.visible_point_cloud_clusters)
            .push_storage_buffer(27, &self.frame_telemetry_buffer)
            .push_storage_buffer(28, &self.device_local_debug_lines_buffer)
            .build();
        command_buffer_builder.push_descriptors(0, pipeline_bind_point, push_descriptors)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use base::device::TestFixtureDevice;
    use jeriya_backend::{elements::camera::Camera, gpu_index_allocator::GpuIndexAllocation, transactions::PushEvent};
    use jeriya_shared::Handle;

    use super::*;

    #[test]
    fn resources() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let (resource_sender, _resource_receiver) = channel();
        let backend_shared = BackendShared::new(&test_fixture_device.device, &Arc::new(Default::default()), resource_sender).unwrap();
        let mut frame = PersistentFrameState::new(0, &test_fixture_device.window.id(), &backend_shared).unwrap();
        let mut transaction = Transaction::new();
        let camera = Camera::new(
            camera::CameraProjection::Orthographic {
                left: -10.0,
                right: 5.0,
                bottom: 2.0,
                top: -3.0,
                near: 4.0,
                far: 11.0,
            },
            debug_info!("my_camera"),
            Handle::zero(),
            GpuIndexAllocation::new_unchecked(0),
        );
        transaction.push_event(transactions::Event::Camera(camera::Event::Insert(camera.clone())));
        frame.push_transaction(transaction);
        frame.process_transactions().unwrap();
        let mut data = vec![shader_interface::Camera::default(); frame.camera_buffer.capacity()];
        frame.camera_buffer.host_visible_buffer().get_memory_unaligned(&mut data).unwrap();
        assert_eq!(data[0].projection_matrix, camera.projection().projection_matrix());
    }
}
