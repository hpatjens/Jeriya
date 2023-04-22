use jeriya_shared::{
    immediate::{CommandBuffer, CommandBufferBuilder},
    winit::window::{Window, WindowId},
    Backend, DebugInfo, RendererConfig, Result,
};

use std::{marker::PhantomData, sync::Arc};

use crate::ResourceContainerBuilder;

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
}
