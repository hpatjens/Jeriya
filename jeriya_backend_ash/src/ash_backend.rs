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
    host_visible_buffer::HostVisibleBuffer,
    immediate_graphics_pipeline::{PushConstants, Topology},
    instance::Instance,
    physical_device::PhysicalDevice,
    queue::{Queue, QueueType},
    semaphore::Semaphore,
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_shared::immediate::{self, TriangleList, TriangleStrip};
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
                let presenter = Presenter::new(&device, window_id, surface, renderer_config.default_desired_swapchain_length)?;
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

            // Build CommandBuffer
            let mut command_buffer = CommandBuffer::new(
                &self.device,
                &self.command_pool,
                debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
            )?;
            let mut command_buffer_builder = CommandBufferBuilder::new(&self.device, &mut command_buffer)?;
            command_buffer_builder
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
                .bind_graphics_pipeline(&presenter.simple_graphics_pipeline);
            self.append_immediate_rendering_commands(window_id, presenter, &mut command_buffer_builder)?;
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
    fn append_immediate_rendering_commands(
        &self,
        window_id: &WindowId,
        presenter: &Presenter,
        command_buffer_builder: &mut CommandBufferBuilder,
    ) -> core::Result<()> {
        let mut immediate_rendering_requests = self.immediate_rendering_requests.lock();
        if let Some(requests) = immediate_rendering_requests.get_mut(window_id) {
            // Collect vertex attributes for all immediate rendering requests
            assert!(!requests.is_empty(), "Vecs should be removed when they are empty");
            let mut data = Vec::new();
            for request in &mut *requests {
                assert!(request.count > 0, "Count must be greater than 0");
                request.count -= 1;
                for command in &request.immediate_command_buffer.commands {
                    match command {
                        ImmediateCommand::LineList(line_list) => data.extend_from_slice(line_list.positions()),
                        ImmediateCommand::LineStrip(line_strip) => data.extend_from_slice(line_strip.positions()),
                        ImmediateCommand::TriangleList(triangle_list) => data.extend_from_slice(triangle_list.positions()),
                        ImmediateCommand::TriangleStrip(triangle_strip) => data.extend_from_slice(triangle_strip.positions()),
                    }
                }
            }
            let vertex_buffer = Arc::new(HostVisibleBuffer::new(
                &self.device,
                data.as_slice(),
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("Immediate-VertexBuffer"),
            )?);
            command_buffer_builder.bind_vertex_buffers(0, &vertex_buffer);

            // Append the draw commands
            let mut first_vertex = 0;
            for immediate_command_buffer in &*requests {
                let mut last_topology = None;
                for command in &immediate_command_buffer.immediate_command_buffer.commands {
                    match command {
                        ImmediateCommand::LineList(line_list) => {
                            if !matches!(last_topology, Some(Topology::LineList)) {
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_line_list);
                            }
                            let push_constants = PushConstants {
                                color: line_list.config().color,
                            };
                            command_buffer_builder.push_constants(&[push_constants])?;
                            command_buffer_builder.set_line_width(line_list.config().line_width);
                            command_buffer_builder.draw_vertices(line_list.positions().len() as u32, first_vertex as u32);
                            first_vertex += line_list.positions().len();
                            last_topology = Some(Topology::LineList);
                        }
                        ImmediateCommand::LineStrip(line_strip) => {
                            if !matches!(last_topology, Some(Topology::LineStrip)) {
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_line_strip);
                            }
                            let push_constants = PushConstants {
                                color: line_strip.config().color,
                            };
                            command_buffer_builder.push_constants(&[push_constants])?;
                            command_buffer_builder.set_line_width(line_strip.config().line_width);
                            command_buffer_builder.draw_vertices(line_strip.positions().len() as u32, first_vertex as u32);
                            first_vertex += line_strip.positions().len();
                            last_topology = Some(Topology::LineStrip);
                        }
                        ImmediateCommand::TriangleList(triangle_list) => {
                            if !matches!(last_topology, Some(Topology::TriangleList)) {
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_triangle_list);
                            }
                            let push_constants = PushConstants {
                                color: triangle_list.config().color,
                            };
                            command_buffer_builder.push_constants(&[push_constants])?;
                            command_buffer_builder.draw_vertices(triangle_list.positions().len() as u32, first_vertex as u32);
                            first_vertex += triangle_list.positions().len();
                            last_topology = Some(Topology::TriangleList);
                        }
                        ImmediateCommand::TriangleStrip(triangle_strip) => {
                            if !matches!(last_topology, Some(Topology::TriangleStrip)) {
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_triangle_strip);
                            }
                            let push_constants = PushConstants {
                                color: triangle_strip.config().color,
                            };
                            command_buffer_builder.push_constants(&[push_constants])?;
                            command_buffer_builder.draw_vertices(triangle_strip.positions().len() as u32, first_vertex as u32);
                            first_vertex += triangle_strip.positions().len();
                            last_topology = Some(Topology::TriangleStrip);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ImmediateCommand {
    LineList(LineList),
    LineStrip(LineStrip),
    TriangleList(TriangleList),
    TriangleStrip(TriangleStrip),
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

    fn push_triangle_lists(&mut self, triangle_lists: &[TriangleList]) -> jeriya_shared::Result<()> {
        for triangle_list in triangle_lists {
            if triangle_list.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::TriangleList(triangle_list.clone()));
        }
        Ok(())
    }

    fn push_triangle_strips(&mut self, triangle_strips: &[TriangleStrip]) -> jeriya_shared::Result<()> {
        for triangle_strip in triangle_strips {
            if triangle_strip.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::TriangleStrip(triangle_strip.clone()));
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
