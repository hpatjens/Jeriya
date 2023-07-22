use jeriya_shared::{
    immediate::{CommandBuffer, CommandBufferBuilder},
    inanimate_mesh::InanimateMeshGroup,
    tracy_client::Client,
    winit::window::{Window, WindowId},
    Backend, Camera, CameraContainerGuard, DebugInfo, Handle, InanimateMeshInstanceContainerGuard, RendererConfig, Result,
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

    /// Returns the [`Backend`] of the `Renderer`
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

    /// Returns a guard to the [`Camera`]s.
    pub fn cameras(&self) -> CameraContainerGuard {
        self.backend.cameras()
    }

    /// Returns the [`InanimateMeshGroup`] of the `Renderer`
    pub fn inanimate_meshes(&self) -> &InanimateMeshGroup {
        &self.backend.inanimate_meshes()
    }

    /// Returns a guard to the [`InanimateMeshInstance`]s
    pub fn inanimate_mesh_instances(&self) -> InanimateMeshInstanceContainerGuard {
        self.backend.inanimate_mesh_instances()
    }

    /// Sets the active camera for the given window.
    pub fn set_active_camera(&self, window_id: WindowId, handle: Handle<Camera>) -> Result<()> {
        self.backend.set_active_camera(window_id, handle)
    }

    /// Returns the active camera for the given window.
    pub fn active_camera(&self, window_id: WindowId) -> Result<Handle<Camera>> {
        self.backend.active_camera(window_id)
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
        // Create a Tracy client before the backend is created because the first thread creating a Client is called "Main thread".
        let _tracy_client = Client::start();

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
        inanimate_mesh::InanimateMeshGroup,
        parking_lot::Mutex,
        winit::window::{Window, WindowId},
        AsDebugInfo, Backend, Camera, CameraContainerGuard, CameraEvent, DebugInfo, EventQueue, Handle,
        ImmediateCommandBufferBuilderHandler, InanimateMeshInstance, InanimateMeshInstanceContainerGuard, InanimateMeshInstanceEvent,
        IndexingContainer, RendererConfig,
    };
    use std::sync::Arc;

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

    struct DummyBackend {
        cameras: Arc<Mutex<IndexingContainer<Camera>>>,
        camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,
        inanimate_mesh_instances: Arc<Mutex<IndexingContainer<InanimateMeshInstance>>>,
        inanimate_mesh_instance_event_queue: Arc<Mutex<EventQueue<InanimateMeshInstanceEvent>>>,
        renderer_config: Arc<RendererConfig>,
        active_camera: Handle<Camera>,
        inanimate_mesh_group: InanimateMeshGroup,
    }
    struct DummyImmediateCommandBufferBuilderHandler(DebugInfo);
    struct DummyImmediateCommandBufferHandler(DebugInfo);
    impl Backend for DummyBackend {
        type BackendConfig = ();

        type ImmediateCommandBufferBuilderHandler = DummyImmediateCommandBufferBuilderHandler;
        type ImmediateCommandBufferHandler = DummyImmediateCommandBufferHandler;

        fn new(
            _renderer_config: jeriya_shared::RendererConfig,
            _backend_config: Self::BackendConfig,
            _windows: &[&Window],
        ) -> jeriya_shared::Result<Self>
        where
            Self: Sized,
        {
            let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
            let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));
            let inanimate_mesh_instances = Arc::new(Mutex::new(IndexingContainer::new()));
            let inanimate_mesh_instance_event_queue = Arc::new(Mutex::new(EventQueue::new()));
            let active_camera = cameras.lock().insert(Camera::default());
            let inanimate_mesh_group = InanimateMeshGroup::new(Arc::new(Mutex::new(EventQueue::new())));
            Ok(Self {
                cameras,
                camera_event_queue,
                renderer_config: Arc::new(RendererConfig::default()),
                active_camera,
                inanimate_mesh_group,
                inanimate_mesh_instances,
                inanimate_mesh_instance_event_queue,
            })
        }

        fn handle_window_resized(&self, _window_id: WindowId) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn handle_render_frame(&self) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn create_immediate_command_buffer_builder(&self, _debug_info: DebugInfo) -> jeriya_shared::Result<CommandBufferBuilder<Self>> {
            Ok(CommandBufferBuilder::new(DummyImmediateCommandBufferBuilderHandler(debug_info!(
                "dummy"
            ))))
        }

        fn render_immediate_command_buffer(&self, _command_buffer: Arc<CommandBuffer<Self>>) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn cameras(&self) -> CameraContainerGuard {
            CameraContainerGuard::new(self.camera_event_queue.lock(), self.cameras.lock(), self.renderer_config.clone())
        }

        fn inanimate_meshes(&self) -> &InanimateMeshGroup {
            &self.inanimate_mesh_group
        }

        fn inanimate_mesh_instances(&self) -> jeriya_shared::InanimateMeshInstanceContainerGuard {
            InanimateMeshInstanceContainerGuard::new(
                self.inanimate_mesh_instance_event_queue.lock(),
                self.inanimate_mesh_instances.lock(),
                self.renderer_config.clone(),
            )
        }

        fn set_active_camera(&self, _window_id: WindowId, _handle: jeriya_shared::Handle<Camera>) -> jeriya_shared::Result<()> {
            Ok(())
        }

        fn active_camera(&self, _window_id: WindowId) -> jeriya_shared::Result<jeriya_shared::Handle<Camera>> {
            Ok(self.active_camera.clone())
        }
    }
    impl ImmediateCommandBufferBuilderHandler for DummyImmediateCommandBufferBuilderHandler {
        type Backend = DummyBackend;

        fn new(_backend: &Self::Backend, debug_info: DebugInfo) -> jeriya_shared::Result<Self>
        where
            Self: Sized,
        {
            Ok(DummyImmediateCommandBufferBuilderHandler(debug_info))
        }

        fn build(self) -> jeriya_shared::Result<Arc<CommandBuffer<Self::Backend>>> {
            Ok(Arc::new(CommandBuffer::new(DummyImmediateCommandBufferHandler(self.0))))
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
    impl AsDebugInfo for DummyImmediateCommandBufferBuilderHandler {
        fn as_debug_info(&self) -> &DebugInfo {
            &self.0
        }
    }
    impl AsDebugInfo for DummyImmediateCommandBufferHandler {
        fn as_debug_info(&self) -> &DebugInfo {
            &self.0
        }
    }
}
