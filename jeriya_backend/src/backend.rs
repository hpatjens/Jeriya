use std::sync::Arc;

use jeriya_content::asset_importer::AssetImporter;
use jeriya_shared::{nalgebra::Matrix4, winit::window::WindowId, AsDebugInfo, DebugInfo, RendererConfig, WindowConfig};

use crate::{
    elements::{self, point_cloud::PointCloud, rigid_mesh::RigidMesh},
    gpu_index_allocator::AllocateGpuIndex,
    immediate::{CommandBuffer, CommandBufferBuilder, ImmediateRenderingFrame, LineList, LineStrip, TriangleList, TriangleStrip},
    instances::{camera_instance::CameraInstance, point_cloud_instance::PointCloudInstance, rigid_mesh_instance::RigidMeshInstance},
    resources::{mesh_attributes::MeshAttributes, point_cloud_attributes::PointCloudAttributes, ResourceReceiver},
    transactions::TransactionProcessor,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend:
    Sized
    + ResourceReceiver
    + TransactionProcessor
    + AllocateGpuIndex<MeshAttributes>
    + AllocateGpuIndex<PointCloudAttributes>
    + AllocateGpuIndex<elements::camera::Camera>
    + AllocateGpuIndex<CameraInstance>
    + AllocateGpuIndex<RigidMesh>
    + AllocateGpuIndex<PointCloud>
    + AllocateGpuIndex<RigidMeshInstance>
    + AllocateGpuIndex<PointCloudInstance>
    + 'static
{
    type BackendConfig: Default;

    /// Creates a new [`Backend`]
    fn new(
        renderer_config: RendererConfig,
        backend_config: Self::BackendConfig,
        asset_importer: Arc<AssetImporter>,
        window_configs: &[WindowConfig],
    ) -> crate::Result<Arc<Self>>
    where
        Self: Sized;

    /// Renders the given [`CommandBuffer`] in the next frame
    fn render_immediate_command_buffer(
        &self,
        immediate_rendering_frame: &ImmediateRenderingFrame,
        command_buffer: CommandBuffer,
    ) -> crate::Result<()>;

    /// Sets the active camera for the given window
    fn set_active_camera(&self, window_id: WindowId, camera_instance: &CameraInstance) -> crate::Result<()>;
}
