mod static_mesh;
mod texture2d;

pub use static_mesh::*;
pub use texture2d::*;

use std::sync::Arc;

use jeriya_shared::{parking_lot::Mutex, DebugInfo};

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource {
    fn new() -> Self
    where
        Self: Sized;
}

/// Collection of [`Resource`]s with a shared commonality
#[derive(Default)]
pub struct ResourceContainer {
    pub debug_info: Option<DebugInfo>,
    pub texture2ds: ResourceGroup<Texture2d>,
    pub static_meshes: ResourceGroup<StaticMesh>,
}

/// Builder for a [`ResourceContainer`]
pub struct ResourceContainerBuilder {
    debug_info: Option<DebugInfo>,
}

impl ResourceContainerBuilder {
    pub(crate) fn new() -> Self {
        Self { debug_info: None }
    }

    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub fn build(self) -> ResourceContainer {
        ResourceContainer {
            debug_info: self.debug_info,
            ..Default::default()
        }
    }
}

/// Collection of [`Resource`]s of the same type
#[derive(Default)]
pub struct ResourceGroup<R> {
    _data: Vec<R>,
}

impl ResourceGroup<Texture2d> {
    pub fn create(&self) -> ResourceBuilder<Texture2d> {
        ResourceBuilder::new(self)
    }
}

/// Builder for a [`Resource`]
pub struct ResourceBuilder<'resgr, R> {
    _resource_group: &'resgr ResourceGroup<R>,
    debug_info: Option<DebugInfo>,
}

impl<'resgr, R> ResourceBuilder<'resgr, R>
where
    R: Resource,
{
    fn new(resource_group: &'resgr ResourceGroup<R>) -> Self {
        Self {
            _resource_group: resource_group,
            debug_info: None,
        }
    }

    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub fn build(self) -> Arc<Mutex<R>> {
        Arc::new(Mutex::new(R::new()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use jeriya_shared::{
        debug_info,
        immediate::{CommandBuffer, CommandBufferBuilder},
        winit::window::{Window, WindowId},
        AsDebugInfo, Backend, DebugInfo, ImmediateCommandBufferBuilder,
    };

    use crate::Renderer;

    struct DummyBackend;
    struct DummyImmediateCommandBufferBuilder(DebugInfo);
    struct DummyImmediateCommandBuffer(DebugInfo);
    impl Backend for DummyBackend {
        type BackendConfig = ();

        type ImmediateCommandBufferBuilder = DummyImmediateCommandBufferBuilder;
        type ImmediateCommandBuffer = DummyImmediateCommandBuffer;

        fn new(
            _renderer_config: jeriya_shared::RendererConfig,
            _backend_config: Self::BackendConfig,
            _windows: &[&Window],
        ) -> jeriya_shared::Result<Self>
        where
            Self: Sized,
        {
            Ok(Self)
        }

        fn handle_window_resized(&self, _window_id: WindowId) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn handle_render_frame(&self) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn create_immediate_command_buffer_builder(&self, _debug_info: DebugInfo) -> jeriya_shared::Result<CommandBufferBuilder<Self>> {
            Ok(CommandBufferBuilder::new(DummyImmediateCommandBufferBuilder(debug_info!("dummy"))))
        }

        fn render_immediate_command_buffer(&self, _command_buffer: Arc<CommandBuffer<Self>>) -> jeriya_shared::Result<()> {
            Ok(())
        }
    }
    impl ImmediateCommandBufferBuilder for DummyImmediateCommandBufferBuilder {
        type Backend = DummyBackend;

        fn new(_backend: &Self::Backend, debug_info: DebugInfo) -> jeriya_shared::Result<Self>
        where
            Self: Sized,
        {
            Ok(DummyImmediateCommandBufferBuilder(debug_info))
        }

        fn build(self) -> jeriya_shared::Result<Arc<CommandBuffer<Self::Backend>>> {
            Ok(Arc::new(CommandBuffer::new(DummyImmediateCommandBuffer(self.0))))
        }

        fn push_line_lists(&mut self, _line_lists: &[jeriya_shared::immediate::LineList]) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn push_line_strips(&mut self, _line_strips: &[jeriya_shared::immediate::LineStrip]) -> jeriya_shared::Result<()> {
            Ok(())
        }
        fn push_triangle_lists(&mut self, _triangle_lists: &[jeriya_shared::immediate::TriangleList]) -> jeriya_shared::Result<()> {
            Ok(())
        }
        fn push_triangle_strips(&mut self, _triangle_strips: &[jeriya_shared::immediate::TriangleStrip]) -> jeriya_shared::Result<()> {
            Ok(())
        }
    }
    impl AsDebugInfo for DummyImmediateCommandBufferBuilder {
        fn as_debug_info(&self) -> &DebugInfo {
            &self.0
        }
    }
    impl AsDebugInfo for DummyImmediateCommandBuffer {
        fn as_debug_info(&self) -> &DebugInfo {
            &self.0
        }
    }

    #[test]
    fn new_resource_group() {
        let renderer = Renderer::<DummyBackend>::builder().build().unwrap();
        let resource_container = renderer
            .create_resource_container()
            .with_debug_info(debug_info!("my_resource_group"))
            .build();
        let texture = resource_container
            .texture2ds
            .create()
            .with_debug_info(debug_info!("my_texture"))
            .build();
        assert_eq!(texture.lock().width(), 0);
    }
}
