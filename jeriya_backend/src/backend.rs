use std::sync::Arc;

use jeriya_shared::{nalgebra::Matrix4, winit::window::WindowId, AsDebugInfo, DebugInfo, RendererConfig, WindowConfig};

use crate::{
    elements::{self, rigid_mesh::RigidMesh},
    gpu_index_allocator::AllocateGpuIndex,
    immediate::{CommandBuffer, CommandBufferBuilder, ImmediateRenderingFrame, LineList, LineStrip, TriangleList, TriangleStrip},
    instances::{camera_instance::CameraInstance, rigid_mesh_instance::RigidMeshInstance},
    resources::{mesh_attributes::MeshAttributes, ResourceReceiver},
    transactions::TransactionProcessor,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend:
    Sized
    + ResourceReceiver
    + TransactionProcessor
    + AllocateGpuIndex<MeshAttributes>
    + AllocateGpuIndex<elements::camera::Camera>
    + AllocateGpuIndex<CameraInstance>
    + AllocateGpuIndex<RigidMesh>
    + AllocateGpuIndex<RigidMeshInstance>
    + 'static
{
    type BackendConfig: Default;

    type ImmediateCommandBufferBuilderHandler: ImmediateCommandBufferBuilderHandler<Backend = Self> + AsDebugInfo;
    type ImmediateCommandBufferHandler: AsDebugInfo;

    /// Creates a new [`Backend`]
    fn new(
        renderer_config: RendererConfig,
        backend_config: Self::BackendConfig,
        window_configs: &[WindowConfig],
    ) -> crate::Result<Arc<Self>>
    where
        Self: Sized;

    /// Creates a new [`CommandBufferBuilder`]
    fn create_immediate_command_buffer_builder(&self, debug_info: DebugInfo) -> crate::Result<CommandBufferBuilder<Self>>;

    /// Renders the given [`CommandBuffer`] in the next frame
    fn render_immediate_command_buffer(
        &self,
        immediate_rendering_frame: &ImmediateRenderingFrame,
        command_buffer: Arc<CommandBuffer<Self>>,
    ) -> crate::Result<()>;

    /// Sets the active camera for the given window
    fn set_active_camera(&self, window_id: WindowId, camera_instance: &CameraInstance) -> crate::Result<()>;
}

pub trait ImmediateCommandBufferBuilderHandler: AsDebugInfo {
    type Backend: Backend;

    /// Create a new [`ImmediateCommandBufferBuilderHandler`]
    fn new(backend: &Self::Backend, debug_info: DebugInfo) -> crate::Result<Self>
    where
        Self: Sized;

    /// Sets the matrix to be used for the following draw calls
    fn matrix(&mut self, matrix: Matrix4<f32>) -> crate::Result<()>;

    /// Push one or more [`LineList`]s to the command buffer
    fn push_line_lists(&mut self, line_lists: &[LineList]) -> crate::Result<()>;

    /// Push one or more [`LineStrip`]s to the command buffer
    fn push_line_strips(&mut self, line_strips: &[LineStrip]) -> crate::Result<()>;

    /// Push one or more [`TriangleList`]s to the command buffer
    fn push_triangle_lists(&mut self, triangle_lists: &[TriangleList]) -> crate::Result<()>;

    /// Push one or more [`TriangleStrip`]s to the command buffer
    fn push_triangle_strips(&mut self, triangle_strips: &[TriangleStrip]) -> crate::Result<()>;

    /// Build the command buffer and submit it for rendering
    fn build(self) -> crate::Result<Arc<CommandBuffer<Self::Backend>>>;
}
