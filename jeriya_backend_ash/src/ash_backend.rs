use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    buffer::BufferUsageFlags,
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    command_pool::{CommandPool, CommandPoolCreateFlags},
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    frame_index::FrameIndex,
    host_visible_buffer::HostVisibleBuffer,
    immediate_graphics_pipeline::ImmediateGraphicsPipeline,
    instance::Instance,
    physical_device::PhysicalDevice,
    queue::{Queue, QueueType},
    semaphore::Semaphore,
    simple_graphics_pipeline::SimpleGraphicsPipeline,
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_shared::immediate::{self};
use jeriya_shared::{
    debug_info,
    immediate::{LineList, LineStrip},
    log::info,
    parking_lot::Mutex,
    winit::window::{Window, WindowId},
    AsDebugInfo, Backend, DebugInfo, ImmediateCommandBufferBuilder, RendererConfig,
};

use crate::presenter::Presenter;

#[derive(Debug)]
struct ImmediateRenderingRequest {
    immediate_command_buffer: AshImmediateCommandBuffer,
    count: usize,
}

pub struct AshBackend {
    presenters: HashMap<WindowId, RefCell<Presenter>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    device: Arc<Device>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    presentation_queue: RefCell<Queue>,
    command_pool: Rc<CommandPool>,
    immediate_rendering_requests: Mutex<HashMap<WindowId, Vec<ImmediateRenderingRequest>>>,
    simple_graphics_pipelines: HashMap<WindowId, SimpleGraphicsPipeline>,
    immediate_graphics_pipelines: HashMap<WindowId, ImmediateGraphicsPipeline>,
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
        let simple_graphics_pipelines = presenters
            .iter()
            .map(|(window_id, presenter)| {
                let presenter = presenter.borrow();
                let graphics_pipeline = SimpleGraphicsPipeline::new(
                    &device,
                    presenter.presenter_resources.render_pass(),
                    presenter.presenter_resources.swapchain(),
                    debug_info!(format!("SimpleGraphicsPipeline-for-Window{:?}", window_id)),
                )?;
                Ok((*window_id, graphics_pipeline))
            })
            .collect::<core::Result<HashMap<_, _>>>()?;
        let immediate_graphics_pipelines = presenters
            .iter()
            .map(|(window_id, presenter)| {
                let presenter = presenter.borrow();
                let graphics_pipeline = ImmediateGraphicsPipeline::new(
                    &device,
                    presenter.presenter_resources.render_pass(),
                    presenter.presenter_resources.swapchain(),
                    debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
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
            immediate_rendering_requests: Mutex::new(HashMap::new()),
            simple_graphics_pipelines,
            immediate_graphics_pipelines,
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

            // Acquire the next swapchain index
            let image_available_semaphore = Arc::new(Semaphore::new(&self.device, debug_info!("image-available-Semaphore"))?);
            let frame_index = presenter
                .presenter_resources
                .swapchain()
                .acquire_next_image(&mut presenter.frame_index(), &image_available_semaphore)?;
            presenter.start_frame(frame_index.clone());
            presenter
                .image_available_semaphore
                .replace(&presenter.frame_index(), image_available_semaphore);

            // Wait for the previous work for the currently occupied frame to finish
            for command_buffer in presenter.rendering_complete_command_buffers.get(&presenter.frame_index()) {
                command_buffer.wait_for_completion()?;
            }
            presenter
                .rendering_complete_command_buffers
                .get_mut(&presenter.frame_index())
                .clear();

            // Prepare rendering complete semaphore
            let main_rendering_complete_semaphore = Arc::new(Semaphore::new(
                &self.device,
                debug_info!("main-CommandBuffer-rendering-complete-Semaphore"),
            )?);
            let rendering_complete_semaphores = presenter.rendering_complete_semaphores.get_mut(&presenter.frame_index());
            rendering_complete_semaphores.clear();
            rendering_complete_semaphores.push(main_rendering_complete_semaphore.clone());
            assert_eq!(
                rendering_complete_semaphores.len(),
                1,
                "There should only be one rendering complete semaphore"
            );

            // Immediate Rendering
            let graphics_pipeline = self
                .simple_graphics_pipelines
                .get(&window_id)
                .expect("no graphics pipeline for window");

            // Build CommandBuffer
            let mut command_buffer = CommandBuffer::new(
                &self.device,
                &self.command_pool,
                debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
            )?;
            let command_buffer_builder = CommandBufferBuilder::new(&self.device, &mut command_buffer)?
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
                .draw_three_vertices();

            let command_buffer_builder = self.append_immediate_rendering_commands(window_id, command_buffer_builder)?;

            command_buffer_builder.end_render_pass()?.end_command_buffer()?;

            // Save CommandBuffer to be able to check whether this frame was completed
            let command_buffer = Arc::new(command_buffer);
            presenter
                .rendering_complete_command_buffers
                .get_mut(&presenter.frame_index())
                .push(command_buffer.clone());

            // Submit immediate rendering
            let image_available_semaphore = presenter
                .image_available_semaphore
                .get(&presenter.frame_index())
                .as_ref()
                .expect("not image available semaphore assigned for the frame");

            // Insert into Queue
            self.presentation_queue.borrow_mut().submit_for_rendering_complete(
                command_buffer,
                &image_available_semaphore,
                &main_rendering_complete_semaphore,
            )?;

            // Present
            presenter.presenter_resources.swapchain().present(
                &presenter.frame_index(),
                &presenter.rendering_complete_semaphores.get(&presenter.frame_index()),
                &self.presentation_queue.borrow(),
            )?;
        }

        // Remove all ImmediateRenderingRequests that don't have to be rendered anymore
        let mut immediate_rendering_requests = self.immediate_rendering_requests.lock();
        for (_window_id, immediate_rendering_requests) in &mut *immediate_rendering_requests {
            *immediate_rendering_requests = immediate_rendering_requests
                .drain(..)
                .filter(|immediate_rendering_request| immediate_rendering_request.count > 0)
                .collect();
        }
        *immediate_rendering_requests = immediate_rendering_requests
            .drain()
            .filter(|(_, immediate_rendering_requests)| !immediate_rendering_requests.is_empty())
            .collect();

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
        let mut guard = self.immediate_rendering_requests.lock();
        for window_id in self.presenters.keys() {
            let immediate_rendering_request = ImmediateRenderingRequest {
                immediate_command_buffer: AshImmediateCommandBuffer {
                    commands: command_buffer.command_buffer().commands.clone(),
                    debug_info: command_buffer.command_buffer().debug_info.clone(),
                },
                count: 1,
            };
            if guard.contains_key(window_id) {
                guard
                    .get_mut(window_id)
                    .expect("failed to find window id")
                    .push(immediate_rendering_request);
            } else {
                guard.insert(*window_id, vec![immediate_rendering_request]);
            }
        }
        Ok(())
    }
}

impl AshBackend {
    fn append_immediate_rendering_commands<'buf>(
        &self,
        window_id: &WindowId,
        command_buffer_builder: CommandBufferBuilder<'buf>,
    ) -> core::Result<CommandBufferBuilder<'buf>> {
        let immediate_graphics_pipeline = self
            .immediate_graphics_pipelines
            .get(window_id)
            .expect("no graphics pipeline for window");
        let mut command_buffer_builder = command_buffer_builder.bind_graphics_pipeline(immediate_graphics_pipeline);
        let mut immediate_rendering_requests = self.immediate_rendering_requests.lock();
        if let Some(requests) = immediate_rendering_requests.get_mut(window_id) {
            assert!(!requests.is_empty(), "Vecs should be removed when they are empty");
            let mut data = Vec::new();
            for request in &mut *requests {
                assert!(request.count > 0, "Count must be greater than 0");
                request.count -= 1;
                for command in &request.immediate_command_buffer.commands {
                    match command {
                        ImmediateCommand::LineList(line_list) => data.extend_from_slice(line_list.positions()),
                        ImmediateCommand::LineStrip(line_strip) => data.extend_from_slice(line_strip.positions()),
                    }
                }
            }
            let vertex_buffer = Arc::new(HostVisibleBuffer::new(
                &self.device,
                data.as_slice(),
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("Immediate-VertexBuffer"),
            )?);
            command_buffer_builder = command_buffer_builder.bind_vertex_buffers(0, &vertex_buffer);
            let mut offset = 0;
            for immediate_command_buffer in &*requests {
                for command in &immediate_command_buffer.immediate_command_buffer.commands {
                    match command {
                        ImmediateCommand::LineList(line_list) => {
                            let first_vertex = offset;
                            offset += line_list.positions().len();
                            command_buffer_builder =
                                command_buffer_builder.draw_vertices(line_list.positions().len() as u32, first_vertex as u32);
                        }
                        ImmediateCommand::LineStrip(line_strip) => {
                            let first_vertex = offset;
                            offset += line_strip.positions().len();
                            command_buffer_builder =
                                command_buffer_builder.draw_vertices(line_strip.positions().len() as u32, first_vertex as u32);
                        }
                    }
                }
            }
        }
        Ok(command_buffer_builder)
    }
}

#[derive(Debug, Clone)]
enum ImmediateCommand {
    LineList(LineList),
    LineStrip(LineStrip),
}

#[derive(Debug)]
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

    fn push_line_lists(&mut self, line_lists: &[LineList]) -> jeriya_shared::Result<()> {
        for line_list in line_lists {
            if line_list.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::LineList(line_list.clone()));
        }
        Ok(())
    }

    fn push_line_strips(&mut self, line_strips: &[LineStrip]) -> jeriya_shared::Result<()> {
        for line_strip in line_strips {
            if line_strip.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::LineStrip(line_strip.clone()));
        }
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
