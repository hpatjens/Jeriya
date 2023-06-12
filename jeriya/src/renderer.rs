use jeriya_shared::{
    immediate::{CommandBuffer, CommandBufferBuilder},
    winit::window::{Window, WindowId},
    Backend, DebugInfo, ObjectContainerBuilder, RendererConfig, ResourceContainerBuilder, Result,
};

use std::{marker::PhantomData, sync::Arc};

/// Instance of the renderer
pub struct Renderer<B>
where
    B: Backend,
{
    backend: B,
}

impl<B> Renderer<B>
where
    B: Backend,
{
    fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Creates a new [`RendererBuilder`] to create an instance of the `Renderer`
    pub fn builder<'a>() -> RendererBuilder<'a, B> {
        RendererBuilder::new()
    }

    pub fn create_resource_container(&self) -> ResourceContainerBuilder {
        ResourceContainerBuilder::new()
    }

    pub fn create_object_container(&self) -> ObjectContainerBuilder {
        ObjectContainerBuilder::new()
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Renders to all `Window`s.
    pub fn render_frame(&self) -> Result<()> {
        self.backend.handle_render_frame()
    }

    /// Has to be called when a window is gets resized.
    pub fn window_resized(&self, window_id: WindowId) -> Result<()> {
        self.backend.handle_window_resized(window_id)
    }

    /// Creates a new [`CommandBufferBuilder`]
    pub fn create_immediate_command_buffer_builder(&self, debug_info: DebugInfo) -> Result<CommandBufferBuilder<B>> {
        self.backend.create_immediate_command_buffer_builder(debug_info)
    }

    /// Renders a [`CommandBuffer`] in the next frame
    pub fn render_immediate_command_buffer(&self, command_buffer: Arc<CommandBuffer<B>>) -> Result<()> {
        self.backend.render_immediate_command_buffer(command_buffer)
    }
}

/// Builder type to create an instance of the [`Renderer`]
pub struct RendererBuilder<'a, B>
where
    B: Backend,
{
    _phantom: PhantomData<B>,
    windows: &'a [&'a Window],
    renderer_config: Option<RendererConfig>,
    backend_config: Option<B::BackendConfig>,
}

impl<'a, B> RendererBuilder<'a, B>
where
    B: Backend,
{
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
            windows: &[],
            renderer_config: None,
            backend_config: None,
        }
    }

    pub fn add_renderer_config(mut self, renderer_config: RendererConfig) -> Self {
        self.renderer_config = Some(renderer_config);
        self
    }

    pub fn add_backend_config(mut self, backend_config: B::BackendConfig) -> Self {
        self.backend_config = Some(backend_config);
        self
    }

    pub fn add_windows(mut self, windows: &'a [&'a Window]) -> Self {
        self.windows = windows;
        self
    }

    pub fn build(self) -> Result<Renderer<B>> {
        let renderer_config = self.renderer_config.unwrap_or(RendererConfig::default());
        let backend_config = self.backend_config.unwrap_or(B::BackendConfig::default());
        let backend = B::new(renderer_config, backend_config, self.windows)?;
        Ok(Renderer::new(backend))
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::{
        debug_info,
        immediate::{CommandBuffer, CommandBufferBuilder},
        winit::window::{Window, WindowId},
        AsDebugInfo, Backend, DebugInfo, ImmediateCommandBufferBuilder,
    };
    use std::sync::Arc;

    use crate::Renderer;

    mod immediate_command_buffer {
        use jeriya_backend_ash::AshBackend;
        use jeriya_shared::{
            debug_info,
            immediate::{LineConfig, LineList},
            nalgebra::Vector3,
        };
        use jeriya_test::create_window;

        use crate::Renderer;

        #[test]
        fn smoke() -> jeriya_shared::Result<()> {
            let window = create_window();
            let renderer = Renderer::<AshBackend>::builder().add_windows(&[&window]).build()?;
            let line_list = LineList::new(
                vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)],
                LineConfig::default(),
            );
            let immediate_command_buffer = renderer
                .create_immediate_command_buffer_builder(debug_info!("my_immediate_command_buffer"))?
                .push_line_lists(&[line_list])?
                .build()?;
            renderer.render_immediate_command_buffer(immediate_command_buffer)?;
            renderer.render_frame()?;
            Ok(())
        }
    }

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
        fn matrix(&mut self, _matrix: jeriya_shared::nalgebra::Matrix4<f32>) -> jeriya_shared::Result<()> {
            Ok(())
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
        let mut resource_container = renderer
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
