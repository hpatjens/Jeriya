use jeriya_backend::{
    elements::{self, point_cloud::PointCloud, rigid_mesh::RigidMesh},
    gpu_index_allocator::ProvideAllocateGpuIndex,
    immediate::{CommandBuffer, CommandBufferBuilder, ImmediateRenderingFrame},
    instances::{camera_instance::CameraInstance, point_cloud_instance::PointCloudInstance, rigid_mesh_instance::RigidMeshInstance},
    resources::{mesh_attributes::MeshAttributes, point_cloud_attributes::PointCloudAttributes, ProvideResourceReceiver},
    transactions::ProvideTransactionProcessor,
    Backend, Result,
};
use jeriya_content::asset_importer::AssetImporter;
use jeriya_shared::{features::info_log_features, tracy_client::Client, winit::window::WindowId, DebugInfo, RendererConfig, WindowConfig};

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
        info_log_features();
        Self { backend }
    }

    /// Creates a new [`RendererBuilder`] to create an instance of the `Renderer`
    ///
    /// # Example
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use jeriya_shared::{
    /// #     FrameRate, RendererConfig, WindowConfig,
    /// #     winit::{
    /// #         dpi::LogicalSize,
    /// #         event::{Event, WindowEvent},
    /// #         event_loop::EventLoop,
    /// #         window::WindowBuilder,
    /// #     }
    /// # };
    /// # use jeriya_backend_ash::AshBackend;
    /// # use jeriya_content::asset_importer::AssetImporter;
    /// let event_loop = EventLoop::new().unwrap();
    /// let window = WindowBuilder::new()
    ///     # .with_visible(false)
    ///     .with_title("Example")
    ///     .with_inner_size(LogicalSize::new(640.0, 480.0))
    ///     .build(&event_loop)
    ///     .unwrap();
    ///
    /// let asset_importer = Arc::new(AssetImporter::default_from("../assets/processed").unwrap());
    ///
    /// // Create Renderer
    /// let renderer = jeriya::Renderer::<AshBackend>::builder()
    ///     .add_renderer_config(RendererConfig::default())
    ///     .add_asset_importer(asset_importer)
    ///     .add_windows(&[
    ///         WindowConfig {
    ///             window: &window,
    ///             frame_rate: FrameRate::Unlimited,
    ///         },
    ///     ])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder<'a>() -> RendererBuilder<'a, B> {
        RendererBuilder::new()
    }

    /// Returns the [`Backend`] that is used by the [`Renderer`]
    pub fn backend(&self) -> &Arc<B> {
        &self.backend
    }

    /// Creates a new [`CommandBufferBuilder`]
    pub fn create_immediate_command_buffer_builder(&self, debug_info: DebugInfo) -> Result<CommandBufferBuilder> {
        Ok(CommandBufferBuilder::new(debug_info))
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
        command_buffer: CommandBuffer,
    ) -> Result<()> {
        self.backend
            .render_immediate_command_buffer(immediate_rendering_frame, command_buffer)
    }

    /// Sets the active camera for the given window.
    pub fn set_active_camera(&self, window_id: WindowId, camera_instance: &CameraInstance) -> Result<()> {
        self.backend.set_active_camera(window_id, camera_instance)
    }
}

impl<B: Backend> ProvideResourceReceiver for Renderer<B> {
    type ResourceReceiver = B;
    fn provide_resource_receiver(&self) -> &Self::ResourceReceiver {
        &self.backend
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<elements::camera::Camera> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<CameraInstance> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<RigidMesh> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<PointCloud> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<RigidMeshInstance> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<PointCloudInstance> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<MeshAttributes> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<B: Backend> ProvideAllocateGpuIndex<PointCloudAttributes> for Renderer<B> {
    type AllocateGpuIndex = B;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(self.backend())
    }
}

impl<'s, B: Backend + 's> ProvideTransactionProcessor<'s> for Renderer<B> {
    type TransactionProcessor = B;
    fn provide_transaction_processor(&'s self) -> &'s Arc<Self::TransactionProcessor> {
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
    asset_importer: Option<Arc<AssetImporter>>,
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
            asset_importer: None,
        }
    }

    pub fn add_renderer_config(mut self, renderer_config: RendererConfig) -> Self {
        self.renderer_config = Some(renderer_config);
        self
    }

    pub fn add_asset_importer(mut self, asset_importer: Arc<AssetImporter>) -> Self {
        self.asset_importer = Some(asset_importer);
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

        let renderer_config = self.renderer_config.unwrap_or_default();
        let backend_config = self.backend_config.unwrap_or_default();
        let asset_importer = self.asset_importer.expect("Asset importer must be set");
        let backend = B::new(renderer_config, backend_config, asset_importer, self.window_configs)?;
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
        elements::{self, point_cloud::PointCloud, rigid_mesh::RigidMesh},
        gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation},
        immediate::{CommandBuffer, ImmediateRenderingFrame},
        instances::{camera_instance::CameraInstance, point_cloud_instance::PointCloudInstance, rigid_mesh_instance::RigidMeshInstance},
        resources::{mesh_attributes::MeshAttributes, point_cloud_attributes::PointCloudAttributes, ResourceEvent, ResourceReceiver},
        transactions::{Transaction, TransactionProcessor},
        Backend,
    };
    use jeriya_content::asset_importer::AssetImporter;
    use jeriya_shared::{winit::window::WindowId, WindowConfig};
    use std::sync::{
        mpsc::{channel, Sender},
        Arc,
    };

    mod immediate_command_buffer {
        use std::sync::Arc;

        use jeriya_backend::immediate::{ImmediateRenderingFrame, LineConfig, LineList};
        use jeriya_backend_ash::AshBackend;
        use jeriya_content::asset_importer::AssetImporter;
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
            let asset_importer = Arc::new(AssetImporter::default_from("../assets/processed").unwrap());
            let renderer = Renderer::<AshBackend>::builder()
                .add_windows(&[window_config])
                .add_asset_importer(asset_importer)
                .build()?;
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
        resource_event_sender: Sender<ResourceEvent>,
    }
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
    impl AllocateGpuIndex<PointCloud> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<PointCloud>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<PointCloud>) {}
    }
    impl AllocateGpuIndex<PointCloudInstance> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<PointCloudInstance>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<PointCloudInstance>) {}
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
    impl AllocateGpuIndex<PointCloudAttributes> for DummyBackend {
        fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<PointCloudAttributes>> {
            None
        }
        fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<PointCloudAttributes>) {}
    }
    impl Backend for DummyBackend {
        type BackendConfig = ();

        fn new(
            _renderer_config: jeriya_shared::RendererConfig,
            _backend_config: Self::BackendConfig,
            _asset_importer: Arc<AssetImporter>,
            _window_configs: &[WindowConfig],
        ) -> jeriya_backend::Result<Arc<Self>>
        where
            Self: Sized,
        {
            Ok(Arc::new(Self {
                resource_event_sender: channel().0,
            }))
        }

        fn render_immediate_command_buffer(
            &self,
            _immediate_rendering_frame: &ImmediateRenderingFrame,
            _command_buffer: CommandBuffer,
        ) -> jeriya_backend::Result<()> {
            Ok(())
        }

        fn set_active_camera(&self, _window_id: WindowId, _camera_instance: &CameraInstance) -> jeriya_backend::Result<()> {
            Ok(())
        }
    }
}
