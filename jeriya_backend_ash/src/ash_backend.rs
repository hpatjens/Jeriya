use std::cell::RefCell;
use std::rc::Rc;
use std::{collections::HashMap, sync::Arc};

use jeriya::Backend;
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
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_shared::{
    log::info,
    winit::window::{Window, WindowId},
    RendererConfig,
};

use crate::presenter::Presenter;

pub struct Ash {
    presenters: HashMap<WindowId, RefCell<Presenter>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    device: Arc<Device>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    presentation_queue: RefCell<Queue>,
    command_pool: Rc<CommandPool>,
}

impl Backend for Ash {
    type BackendConfig = Config;

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
        let command_pool = CommandPool::new(&device, &presentation_queue, CommandPoolCreateFlags::ResetCommandBuffer)?;

        Ok(Self {
            device,
            _validation_layer_callback: validation_layer_callback,
            _entry: entry,
            _instance: instance,
            presenters,
            _surfaces: surfaces,
            presentation_queue: RefCell::new(presentation_queue),
            command_pool,
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

        for presenter in self.presenters.values() {
            let presenter = &mut *presenter.borrow_mut();

            // Acquire the next swapchain index
            let image_available_semaphore = Semaphore::new(&self.device)?;
            presenter.frame_index = presenter
                .presenter_resources
                .swapchain()
                .acquire_next_image(&mut presenter.frame_index, &image_available_semaphore)?;
            presenter
                .image_available_semaphore
                .replace(&presenter.frame_index, image_available_semaphore);

            // Build CommandBuffer
            let command_buffer = CommandBuffer::new(&self.device, &self.command_pool)?;
            CommandBufferBuilder::new(&self.device, &command_buffer)?
                .begin_command_buffer_for_one_time_submit()?
                .depth_pipeline_barrier(
                    presenter
                        .presenter_resources
                        .depth_buffers()
                        .depth_buffers
                        .get(&presenter.frame_index),
                )?
                .begin_render_pass(
                    presenter.presenter_resources.swapchain(),
                    presenter.presenter_resources.render_pass(),
                    (
                        presenter.presenter_resources.framebuffers(),
                        presenter.frame_index.swapchain_index(),
                    ),
                )?
                .end_render_pass()?
                .end_command_buffer()?;

            // Insert into Queue
            let image_available_semaphore = presenter
                .image_available_semaphore
                .get(&presenter.frame_index)
                .as_ref()
                .expect("not image available semaphore assigned for the frame");
            self.presentation_queue.borrow_mut().submit_with_wait_at_color_attachment_output(
                command_buffer,
                &image_available_semaphore,
                presenter.rendering_complete_semaphore.get(&presenter.frame_index),
            )?;

            // Present
            presenter.presenter_resources.swapchain().present(
                &presenter.frame_index,
                &presenter.rendering_complete_semaphore.get(&presenter.frame_index),
                &self.presentation_queue.borrow(),
            )?;
        }
        Ok(())
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
            Ash::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            Ash::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn empty_windows_none() {
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            assert!(matches!(
                Ash::new(renderer_config, backend_config, &[]),
                Err(jeriya_shared::Error::ExpectedWindow)
            ));
        }
    }
}
