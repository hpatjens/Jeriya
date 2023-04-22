use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    command_pool::CommandPool,
    command_pool::CommandPoolCreateFlags,
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    instance::Instance,
    physical_device::PhysicalDevice,
    queue::{Queue, QueueType},
    semaphore::Semaphore,
    simple_graphics_pipeline::SimpleGraphicsPipeline,
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_shared::immediate;
use jeriya_shared::{
    debug_info,
    immediate::{LineList, LineStrip},
    log::info,
    parking_lot::Mutex,
    winit::window::{Window, WindowId},
    AsDebugInfo, Backend, DebugInfo, ImmediateCommandBufferBuilder, RendererConfig,
};

use crate::presenter::Presenter;

pub struct AshBackend {
    presenters: HashMap<WindowId, RefCell<Presenter>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    device: Arc<Device>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    presentation_queue: RefCell<Queue>,
    command_pool: Rc<CommandPool>,
    immediate_command_buffer_builders: Mutex<Vec<AshImmediateCommandBuffer>>,
    graphics_pipelines: HashMap<WindowId, SimpleGraphicsPipeline>,
}

impl Backend for AshBackend {
    type BackendConfig = Config;

    type ImmediateCommandBufferBuilder = AshImmediateCommandBufferBuilder;
    type ImmediateCommandBuffer = AshImmediateCommandBuffer;

    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> jeriya_shared::Result<Self>
    where
        Self: Sized,
    {
        if windows.is_empty() {
            return Err(jeriya_shared::Error::ExpectedWindow);
        }

        info!("Creating Vulkan Entry");
        let entry = Entry::new()?;

        info!("Creating Vulkan Instance");
        let application_name = renderer_config.application_name.unwrap_or(env!("CARGO_PKG_NAME").to_owned());
        let instance = Instance::new(
            &entry,
            &application_name,
            matches!(backend_config.validation_layer, ValidationLayerConfig::Enabled { .. }),
        )?;

        // Validation Layer Callback
        let validation_layer_callback = match backend_config.validation_layer {
            ValidationLayerConfig::Disabled => {
                info!("Skipping validation layer callback setup");
                None
            }
            ValidationLayerConfig::Enabled { panic_on_message } => {
                info!("Setting up validation layer callback");
                set_panic_on_message(panic_on_message);
                Some(ValidationLayerCallback::new(&entry, &instance)?)
            }
        };

        // Surfaces
        let windows = windows.iter().map(|window| (window.id(), window)).collect::<HashMap<_, _>>();
        let surfaces = windows
            .iter()
            .map(|(window_id, window)| {
                info!("Creating Surface for window {window_id:?}");
                let surface = Surface::new(&entry, &instance, window)?;
                Ok((*window_id, surface))
            })
            .collect::<core::Result<HashMap<WindowId, Arc<Surface>>>>()?;

        // Physical Device
        info!("Creating PhysicalDevice");
        let physical_device = PhysicalDevice::new(&instance, surfaces.values())?;

        // Device
        info!("Creating Device");
        let device = Device::new(physical_device, &instance)?;

        // Presentation Queue
        let presentation_queue = Queue::new(&device, QueueType::Presentation)?;

        // Presenters
        let presenters = surfaces
            .iter()
            .map(|(window_id, surface)| {
                info!("Creating presenter for window {window_id:?}");
                let presenter = Presenter::new(&device, surface, renderer_config.default_desired_swapchain_length)?;
                Ok((*window_id, RefCell::new(presenter)))
            })
            .collect::<core::Result<HashMap<_, _>>>()?;

        // CommandPool
        let command_pool = CommandPool::new(
            &device,
            &presentation_queue,
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;

        // Graphics Pipeline
        let graphics_pipelines = presenters
            .iter()
            .map(|(window_id, presenter)| {
                let presenter = presenter.borrow();
                let graphics_pipeline = SimpleGraphicsPipeline::new(
                    &device,
                    presenter.presenter_resources.render_pass(),
                    presenter.presenter_resources.swapchain(),
                    debug_info!(format!("GraphicsPipeline-for-Window{:?}", window_id)),
                )?;
                Ok((*window_id, graphics_pipeline))
            })
            .collect::<core::Result<HashMap<_, _>>>()?;

        Ok(Self {
            device,
            _validation_layer_callback: validation_layer_callback,
            _entry: entry,
            _instance: instance,
            presenters,
            _surfaces: surfaces,
            presentation_queue: RefCell::new(presentation_queue),
            command_pool,
            immediate_command_buffer_builders: Mutex::new(Vec::new()),
            graphics_pipelines,
        })
    }

    fn handle_window_resized(&self, window_id: WindowId) -> jeriya_shared::Result<()> {
        let mut presenter = self
            .presenters
            .get(&window_id)
            .ok_or_else(|| core::Error::UnknownWindowId(window_id))?
            .borrow_mut();
        presenter.recreate()?;
        Ok(())
    }

    fn handle_render_frame(&self) -> jeriya_shared::Result<()> {
        self.presentation_queue.borrow_mut().update()?;

        for (window_id, presenter) in &self.presenters {
            let presenter = &mut *presenter.borrow_mut();

            let graphics_pipeline = self.graphics_pipelines.get(&window_id).expect("no graphics pipeline for window");

            // Wait for the oldest frame to finish
            if let Some(oldest_frame) = presenter.oldest_frame_index() {
                if let Some(command_buffer) = presenter.rendering_complete_command_buffer.get(&oldest_frame) {
                    command_buffer.wait_for_completion()?;
                }
            }

            // Acquire the next swapchain index
            let image_available_semaphore = Semaphore::new(&self.device, debug_info!("image-available-Semaphore"))?;
            let frame_index = presenter
                .presenter_resources
                .swapchain()
                .acquire_next_image(&mut presenter.frame_index(), &image_available_semaphore)?;
            presenter.start_frame(frame_index);
            presenter
                .image_available_semaphore
                .replace(&presenter.frame_index(), image_available_semaphore);

            // Build CommandBuffer
            let mut command_buffer = CommandBuffer::new(
                &self.device,
                &self.command_pool,
                debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
            )?;
            CommandBufferBuilder::new(&self.device, &mut command_buffer)?
                .begin_command_buffer_for_one_time_submit()?
                .depth_pipeline_barrier(
                    presenter
                        .presenter_resources
                        .depth_buffers()
                        .depth_buffers
                        .get(&presenter.frame_index()),
                )?
                .begin_render_pass(
                    presenter.presenter_resources.swapchain(),
                    presenter.presenter_resources.render_pass(),
                    (
                        presenter.presenter_resources.framebuffers(),
                        presenter.frame_index().swapchain_index(),
                    ),
                )?
                .bind_graphics_pipeline(graphics_pipeline)
                .draw_three_vertices()
                .end_render_pass()?
                .end_command_buffer()?;

            // Save CommandBuffer to be able to check whether this frame was completed
            let command_buffer = Arc::new(command_buffer);
            presenter
                .rendering_complete_command_buffer
                .replace(&presenter.frame_index(), command_buffer.clone());

            // Insert into Queue
            let image_available_semaphore = presenter
                .image_available_semaphore
                .get(&presenter.frame_index())
                .as_ref()
                .expect("not image available semaphore assigned for the frame");
            self.presentation_queue.borrow_mut().submit_for_rendering_complete(
                command_buffer,
                &image_available_semaphore,
                presenter.rendering_complete_semaphore.get(&presenter.frame_index()),
            )?;

            // Present
            presenter.presenter_resources.swapchain().present(
                &presenter.frame_index(),
                &presenter.rendering_complete_semaphore.get(&presenter.frame_index()),
                &self.presentation_queue.borrow(),
            )?;
        }
        Ok(())
    }

    fn create_immediate_command_buffer_builder(
        &self,
        debug_info: DebugInfo,
    ) -> jeriya_shared::Result<immediate::CommandBufferBuilder<Self>> {
        let command_buffer_builder = AshImmediateCommandBufferBuilder::new(self, debug_info)?;
        Ok(immediate::CommandBufferBuilder::new(command_buffer_builder))
    }

    fn render_immediate_command_buffer(&self, command_buffer: Arc<immediate::CommandBuffer<Self>>) -> jeriya_shared::Result<()> {
        let mut guard = self.immediate_command_buffer_builders.lock();
        guard.push(AshImmediateCommandBuffer {
            commands: command_buffer.command_buffer().commands.clone(),
            debug_info: command_buffer.command_buffer().debug_info.clone(),
        });
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ImmediateCommand {
    LineLists(Vec<LineList>),
    LineStrips(Vec<LineStrip>),
}

pub struct AshImmediateCommandBuffer {
    commands: Vec<ImmediateCommand>,
    debug_info: DebugInfo,
}

impl AsDebugInfo for AshImmediateCommandBuffer {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

pub struct AshImmediateCommandBufferBuilder {
    commands: Vec<ImmediateCommand>,
    debug_info: DebugInfo,
}

impl ImmediateCommandBufferBuilder for AshImmediateCommandBufferBuilder {
    type Backend = AshBackend;

    fn new(_backend: &Self::Backend, debug_info: DebugInfo) -> jeriya_shared::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            commands: Vec::new(),
            debug_info,
        })
    }

    fn push_line_lists(&mut self, lines: &[LineList]) -> jeriya_shared::Result<()> {
        self.commands.push(ImmediateCommand::LineLists(lines.to_vec()));
        Ok(())
    }

    fn push_line_strips(&mut self, line_strips: &[LineStrip]) -> jeriya_shared::Result<()> {
        self.commands.push(ImmediateCommand::LineStrips(line_strips.to_vec()));
        Ok(())
    }

    fn build(self) -> jeriya_shared::Result<Arc<immediate::CommandBuffer<Self::Backend>>> {
        let command_buffer = AshImmediateCommandBuffer {
            commands: self.commands,
            debug_info: self.debug_info,
        };
        Ok(Arc::new(immediate::CommandBuffer::new(command_buffer)))
    }
}

impl AsDebugInfo for AshImmediateCommandBufferBuilder {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod backend_new {
        use jeriya_test::create_window;

        use super::*;

        #[test]
        fn smoke() {
            let window = create_window();
            let renderer_config = RendererConfig {
                application_name: Some("my_application".to_owned()),
                ..RendererConfig::default()
            };
            let backend_config = Config::default();
            AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn empty_windows_none() {
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            assert!(matches!(
                AshBackend::new(renderer_config, backend_config, &[]),
                Err(jeriya_shared::Error::ExpectedWindow)
            ));
        }
    }

    mod render_frame {
        use jeriya_test::create_window;

        use super::*;

        #[test]
        fn smoke() {
            let window = create_window();
            let renderer_config = RendererConfig {
                application_name: Some("my_application".to_owned()),
                ..RendererConfig::default()
            };
            let backend_config = Config::default();
            let backend = AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
            backend.handle_render_frame().unwrap();
        }
    }
}
