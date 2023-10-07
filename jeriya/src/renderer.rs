use jeriya_shared::{tracy_client::Client, winit::window::WindowId, DebugInfo, Handle, RendererConfig, WindowConfig};

use jeriya_backend::{
    elements::{self, rigid_mesh::RigidMesh},
    gpu_index_allocator::IntoAllocateGpuIndex,
    immediate::{CommandBuffer, CommandBufferBuilder, ImmediateRenderingFrame},
    instances::{camera_instance::CameraInstance, rigid_mesh_instance::RigidMeshInstance},
    resources::{mesh_attributes::MeshAttributes, IntoResourceReceiver},
    transactions::IntoTransactionProcessor,
    Backend, Camera, CameraContainerGuard, Result,
};

use std::{
    marker::PhantomData,
    sync::{Arc, Weak},
};

/// Instance of the renderer
pub struct Renderer<B>
where
    B: Backend,
{
    backend: Arc<B>,
}

impl<B> Renderer<B>
where
    B: Backend,
{
    fn new(backend: Arc<B>) -> Self {
        Self { backend }
    }

    /// Creates a new [`RendererBuilder`] to create an instance of the `Renderer`
    pub fn builder<'a>() -> RendererBuilder<'a, B> {
        RendererBuilder::new()
    }

    /// Returns the [`Backend`] that is used by the [`Renderer`]
    pub fn backend(&self) -> &Arc<B> {
        &self.backend
    }

    /// Creates a new [`CommandBufferBuilder`]
    pub fn create_immediate_command_buffer_builder(&self, debug_info: DebugInfo) -> Result<CommandBufferBuilder<B>> {
        self.backend.create_immediate_command_buffer_builder(debug_info)
    }

    /// Renders a [`CommandBuffer`] for the given [`ImmediateRenderingFrame`].
    ///
    /// The rendering frequencies for the surfaces might vary and are is not locked to
    /// the update frequency. This means that one window might be rendered at 60 FPS
    /// while another window is rendered at 144 FPS. The update frequency might be lower
    /// than either one of them at e.g. 30 FPS. This means that the renderer must have a
    /// way to determine for how many frames the [`CommandBuffer`] should be rendered.
    /// When rendering an [`ImmediateCommandBuffer`] only for the following frame, the
    /// image might flicker when the update framerate is lower than the rendering framerate.
    ///
    /// To solve this problem, the [`ImmediateRenderingFrame`] is used. It determines for
    /// how many frames the [`CommandBuffer`] should be rendered.
    pub fn render_immediate_command_buffer(
        &self,
        immediate_rendering_frame: &ImmediateRenderingFrame,
        command_buffer: Arc<CommandBuffer<B>>,
    ) -> Result<()> {
        self.backend
            .render_immediate_command_buffer(immediate_rendering_frame, command_buffer)
    }

    /// Returns a guard to the [`Camera`]s.
    pub fn cameras(&self) -> CameraContainerGuard {
        self.backend.cameras()
    }

    /// Sets the active camera for the given window.
    pub fn set_active_camera(&self, window_id: WindowId, camera_instance: &CameraInstance) -> Result<()> {
        self.backend.set_active_camera(window_id, camera_instance)
    }
}

impl<B: Backend> IntoResourceReceiver for Renderer<B> {
    type ResourceReceiver = B;
    fn into_resource_receiver(&self) -> &Self::ResourceReceiver {
        &self.backend
    }
}

impl<B: Backend> IntoAllocateGpuIndex<elements::camera::Camera> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn into_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> IntoAllocateGpuIndex<CameraInstance> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn into_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> IntoAllocateGpuIndex<RigidMesh> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn into_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> IntoAllocateGpuIndex<RigidMeshInstance> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn into_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> IntoAllocateGpuIndex<MeshAttributes> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn into_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<'s, B: Backend + 's> IntoTransactionProcessor<'s> for Renderer<B> {
    type TransactionProcessor = B;
    fn into_transaction_processor(&'s self) -> &'s Arc<Self::TransactionProcessor> {
        &self.backend
    }
}

/// Builder type to create an instance of the [`Renderer`]
pub struct RendererBuilder<'a, B>
where
    B: Backend,
{
    _phantom: PhantomData<B>,
    window_configs: &'a [WindowConfig<'a>],
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
            window_configs: &[],
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

    pub fn add_windows(mut self, window_configs: &'a [WindowConfig<'a>]) -> Self {
        self.window_configs = window_configs;
        self
    }

    pub fn build(self) -> Result<Arc<Renderer<B>>> {
        // Create a Tracy client before the backend is created because the first thread creating a Client is called "Main thread".
        let _tracy_client = Client::start();

        // Run deadlock detection in a separate thread.
        #[cfg(feature = "deadlock_detection")]
        {
            use std::thread;
            thread::spawn(move || run_deadlock_detection());
        }

        let renderer_config = self.renderer_config.unwrap_or(RendererConfig::default());
        let backend_config = self.backend_config.unwrap_or(B::BackendConfig::default());
        let backend = B::new(renderer_config, backend_config, self.window_configs)?;
        Ok(Arc::new(Renderer::new(backend)))
    }
}

#[cfg(feature = "deadlock_detection")]
fn run_deadlock_detection() {
    use jeriya_shared::{
        log::{error, info},
        parking_lot::deadlock,
    };
    use std::{thread, time::Duration};

    info!("Deadlock detection thread started");

    loop {
        thread::sleep(Duration::from_secs(1));
        let deadlocks = deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        error!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            error!("Deadlock #{}", i);
            for t in threads {
                error!("Thread Id {:#?}", t.thread_id());
                error!("{:#?}", t.backtrace());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use jeriya_backend::{
        elements::{self, rigid_mesh::RigidMesh},
        gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation},
        immediate::{CommandBuffer, CommandBufferBuilder, ImmediateRenderingFrame},
        instances::{camera_instance::CameraInstance, rigid_mesh_instance::RigidMeshInstance},
        resources::{mesh_attributes::MeshAttributes, ResourceEvent, ResourceReceiver},
        transactions::{Transaction, TransactionProcessor},
        Backend, Camera, CameraContainerGuard, CameraEvent, ImmediateCommandBufferBuilderHandler,
    };
    use jeriya_shared::{
        debug_info, parking_lot::Mutex, winit::window::WindowId, AsDebugInfo, DebugInfo, EventQueue, Handle, IndexingContainer,
        RendererConfig, WindowConfig,
    };
    use std::sync::{
        mpsc::{channel, Sender},
        Arc,
    };

    mod immediate_command_buffer {
        use jeriya_backend::immediate::{ImmediateRenderingFrame, LineConfig, LineList};
        use jeriya_backend_ash::AshBackend;
        use jeriya_shared::{debug_info, nalgebra::Vector3, FrameRate, WindowConfig};
        use jeriya_test::create_window;

        use crate::Renderer;

        #[test]
        fn smoke() -> jeriya_backend::Result<()> {
            let window = create_window();
            let window_config = WindowConfig {
                window: &window,
                frame_rate: FrameRate::Unlimited,
            };
            let renderer = Renderer::<AshBackend>::builder().add_windows(&[window_config]).build()?;
            let line_list = LineList::new(
                vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)],
                LineConfig::default(),
            );
            let immediate_rendering_frame = ImmediateRenderingFrame::new("my_main_loop", 0, jeriya_backend::immediate::Timeout::Infinite);
            let immediate_command_buffer = renderer
                .create_immediate_command_buffer_builder(debug_info!("my_immediate_command_buffer"))?
                .push_line_lists(&[line_list])?
                .build()?;
            renderer.render_immediate_command_buffer(&immediate_rendering_frame, immediate_command_buffer)?;
            Ok(())
        }
    }

    struct DummyBackend {
        cameras: Arc<Mutex<IndexingContainer<Camera>>>,
        camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,
        renderer_config: Arc<RendererConfig>,
        active_camera: Handle<Camera>,
        resource_event_sender: Sender<ResourceEvent>,
    }
    struct DummyImmediateCommandBufferBuilderHandler(DebugInfo);
    struct DummyImmediateCommandBufferHandler(DebugInfo);
    impl ResourceReceiver for DummyBackend {
        fn sender(&self) -> &Sender<ResourceEvent> {
            &self.resource_event_sender
        }
    }
    impl TransactionProcessor for DummyBackend {
        fn process(&self, _transaction: Transaction) {}
    }
    impl AllocateGpuIndex<elements::camera::Camera> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<elements::camera::Camera>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<elements::camera::Camera>) {}
    }
    impl AllocateGpuIndex<CameraInstance> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<CameraInstance>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<CameraInstance>) {}
    }
    impl AllocateGpuIndex<RigidMesh> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<RigidMesh>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<RigidMesh>) {}
    }
    impl AllocateGpuIndex<RigidMeshInstance> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<RigidMeshInstance>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<RigidMeshInstance>) {}
    }
    impl AllocateGpuIndex<MeshAttributes> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<MeshAttributes>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<MeshAttributes>) {}
    }
    impl Backend for DummyBackend {
        type BackendConfig = ();

        type ImmediateCommandBufferBuilderHandler = DummyImmediateCommandBufferBuilderHandler;
        type ImmediateCommandBufferHandler = DummyImmediateCommandBufferHandler;

        fn new(
            _renderer_config: jeriya_shared::RendererConfig,
            _backend_config: Self::BackendConfig,
            _window_configs: &[WindowConfig],
        ) -> jeriya_backend::Result<Arc<Self>>
        where
            Self: Sized,
        {
            let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
            let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));
            let active_camera = cameras.lock().insert(Camera::default());
            Ok(Arc::new(Self {
                cameras,
                camera_event_queue,
                renderer_config: Arc::new(RendererConfig::default()),
                active_camera,
                resource_event_sender: channel().0,
            }))
        }

        fn create_immediate_command_buffer_builder(&self, _debug_info: DebugInfo) -> jeriya_backend::Result<CommandBufferBuilder<Self>> {
            Ok(CommandBufferBuilder::new(DummyImmediateCommandBufferBuilderHandler(debug_info!(
                "dummy"
            ))))
        }

        fn render_immediate_command_buffer(
            &self,
            _immediate_rendering_frame: &ImmediateRenderingFrame,
            _command_buffer: Arc<CommandBuffer<Self>>,
        ) -> jeriya_backend::Result<()> {
            Ok(())
        }

        fn cameras(&self) -> CameraContainerGuard {
            CameraContainerGuard::new(self.camera_event_queue.lock(), self.cameras.lock(), self.renderer_config.clone())
        }

        fn set_active_camera(&self, _window_id: WindowId, _handle: jeriya_shared::Handle<Camera>) -> jeriya_backend::Result<()> {
            Ok(())
        }

        fn active_camera(&self, _window_id: WindowId) -> jeriya_backend::Result<jeriya_shared::Handle<Camera>> {
            Ok(self.active_camera.clone())
        }
    }

    impl ImmediateCommandBufferBuilderHandler for DummyImmediateCommandBufferBuilderHandler {
        type Backend = DummyBackend;

        fn new(_backend: &Self::Backend, debug_info: DebugInfo) -> jeriya_backend::Result<Self>
        where
            Self: Sized,
        {
            Ok(DummyImmediateCommandBufferBuilderHandler(debug_info))
        }

        fn build(self) -> jeriya_backend::Result<Arc<CommandBuffer<Self::Backend>>> {
            Ok(Arc::new(CommandBuffer::new(DummyImmediateCommandBufferHandler(self.0))))
        }
        fn matrix(&mut self, _matrix: jeriya_shared::nalgebra::Matrix4<f32>) -> jeriya_backend::Result<()> {
            Ok(())
        }
        fn push_line_lists(&mut self, _line_lists: &[jeriya_backend::immediate::LineList]) -> jeriya_backend::Result<()> {
            Ok(())
        }

        fn push_line_strips(&mut self, _line_strips: &[jeriya_backend::immediate::LineStrip]) -> jeriya_backend::Result<()> {
            Ok(())
        }
        fn push_triangle_lists(&mut self, _triangle_lists: &[jeriya_backend::immediate::TriangleList]) -> jeriya_backend::Result<()> {
            Ok(())
        }
        fn push_triangle_strips(&mut self, _triangle_strips: &[jeriya_backend::immediate::TriangleStrip]) -> jeriya_backend::Result<()> {
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
