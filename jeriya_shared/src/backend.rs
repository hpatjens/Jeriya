use std::sync::Arc;

use nalgebra::Matrix4;

use crate::{
    immediate::{CommandBuffer, CommandBufferBuilder, LineList, LineStrip, TriangleList, TriangleStrip},
    winit::window::{Window, WindowId},
    AsDebugInfo, Camera, DebugInfo, Handle, ObjectContainer, ObjectGroupGuard, RendererConfig,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend: Sized {
    type BackendConfig: Default;

    type ImmediateCommandBufferBuilderHandler: ImmediateCommandBufferBuilderHandler<Backend = Self> + AsDebugInfo;
    type ImmediateCommandBufferHandler: AsDebugInfo;

    type ObjectContainerHandler: ObjectContainerHandler<Backend = Self> + AsDebugInfo;
    type ObjectGroupGuardHandler<'a, T>: ObjectGroupGuardHandler<T> + AsDebugInfo
    where
        T: 'a;

    /// Creates a new [`Backend`]
    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> crate::Result<Self>
    where
        Self: Sized;

    /// Is called when a window is resized so that the backend can respond.
    fn handle_window_resized(&self, window_id: WindowId) -> crate::Result<()>;

    /// Is called when rendering is requested.
    fn handle_render_frame(&self) -> crate::Result<()>;

    /// Creates a new [`CommandBufferBuilder`]
    fn create_immediate_command_buffer_builder(&self, debug_info: DebugInfo) -> crate::Result<CommandBufferBuilder<Self>>;

    /// Renders the given [`CommandBuffer`] in the next frame
    fn render_immediate_command_buffer(&self, command_buffer: Arc<CommandBuffer<Self>>) -> crate::Result<()>;

    /// Creates a new [`ObjectContainer`]
    fn create_object_container(&self, debug_info: DebugInfo) -> crate::Result<ObjectContainer<Self>>;
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

pub trait ObjectContainerHandler: AsDebugInfo {
    type Backend: Backend;

    fn new(backend: &Self::Backend, debug_info: DebugInfo) -> crate::Result<Self>
    where
        Self: Sized;

    fn cameras(&self) -> ObjectGroupGuard<Camera, Self::Backend>;
}

pub trait ObjectGroupGuardHandler<T> {
    type Backend: Backend;

    /// Inserts a new object and returns a [`Handle`] to it.
    fn insert(&mut self, object: T) -> Handle<T>;

    /// Removes the object with the given handle and returns it.
    ///
    /// # Notes
    ///
    /// It is only available for T: Default to prevent unsafe code in the
    /// [`IndexingContainer`]. This limitation will be lifted.
    fn remove(&mut self, handle: &Handle<T>) -> Option<T>
    where
        T: Default;

    /// Returns a reference to the object with the given [`Handle`].
    fn get(&self, handle: &Handle<T>) -> Option<&T>;

    /// Returns a mutable reference to the object with the given [`Handle`].
    fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T>;
}
