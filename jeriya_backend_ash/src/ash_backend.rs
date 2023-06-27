use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::{
    ash_immediate::{AshImmediateCommandBufferBuilderHandler, AshImmediateCommandBufferHandler},
    ash_shared_backend::AshSharedBackend,
    presenter::Presenter,
};
use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    command_pool::{CommandPool, CommandPoolCreateFlags},
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    instance::Instance,
    physical_device::PhysicalDevice,
    queue::{Queue, QueueType},
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_shared::{
    debug_info, immediate,
    log::info,
    parking_lot::Mutex,
    winit::window::{Window, WindowId},
    Backend, Camera, CameraContainerGuard, DebugInfo, EventQueue, Handle, ImmediateCommandBufferBuilderHandler, IndexingContainer,
    RendererConfig,
};

#[derive(Debug)]
pub struct ImmediateRenderingRequest {
    pub immediate_command_buffer: AshImmediateCommandBufferHandler,
    pub count: usize,
}

pub struct AshBackend {
    presenters: HashMap<WindowId, RefCell<Presenter>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    shared_backend: AshSharedBackend,
}

impl Backend for AshBackend {
    type BackendConfig = Config;

    type ImmediateCommandBufferBuilderHandler = AshImmediateCommandBufferBuilderHandler;
    type ImmediateCommandBufferHandler = AshImmediateCommandBufferHandler;

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

        let windows = windows.iter().map(|window| (window.id(), window)).collect::<HashMap<_, _>>();
        let surfaces = windows
            .iter()
            .map(|(window_id, window)| {
                info!("Creating Surface for window {window_id:?}");
                let surface = Surface::new(&entry, &instance, window)?;
                Ok((*window_id, surface))
            })
            .collect::<core::Result<HashMap<WindowId, Arc<Surface>>>>()?;

        info!("Creating PhysicalDevice");
        let physical_device = PhysicalDevice::new(&instance, surfaces.values())?;

        info!("Creating Device");
        let device = Device::new(physical_device, &instance)?;
        // Presentation Queue
        let presentation_queue = Queue::new(&device, QueueType::Presentation)?;

        info!("Creating Cameras");
        let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
        let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        let presenters = surfaces
            .iter()
            .map(|(window_id, surface)| {
                info!("Creating presenter for window {window_id:?}");
                let presenter = Presenter::new(
                    &device,
                    window_id,
                    surface,
                    renderer_config.default_desired_swapchain_length,
                    &cameras,
                    &camera_event_queue,
                )?;
                Ok((*window_id, RefCell::new(presenter)))
            })
            .collect::<core::Result<HashMap<_, _>>>()?;

        info!("Creating CommandPool");
        let command_pool = CommandPool::new(
            &device,
            &presentation_queue,
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;

        let shared_backend = AshSharedBackend::new(
            device,
            RefCell::new(presentation_queue),
            command_pool,
            Mutex::new(HashMap::new()),
            cameras,
            camera_event_queue,
        );

        Ok(Self {
            _entry: entry,
            _instance: instance,
            _surfaces: surfaces,
            _validation_layer_callback: validation_layer_callback,
            presenters,
            shared_backend,
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
        self.shared_backend.presentation_queue.borrow_mut().update()?;

        // Render on all surfaces
        for (window_id, presenter) in &self.presenters {
            let presenter = &mut *presenter.borrow_mut();
            presenter.render_frame(
                &self.shared_backend.device,
                window_id,
                &self.shared_backend.command_pool,
                &self.shared_backend.presentation_queue,
                &self.shared_backend.immediate_rendering_requests,
            )?;
        }

        // Remove all ImmediateRenderingRequests that don't have to be rendered anymore
        let mut immediate_rendering_requests = self.shared_backend.immediate_rendering_requests.lock();
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
        let command_buffer_builder = AshImmediateCommandBufferBuilderHandler::new(self, debug_info)?;
        Ok(immediate::CommandBufferBuilder::new(command_buffer_builder))
    }

    fn render_immediate_command_buffer(&self, command_buffer: Arc<immediate::CommandBuffer<Self>>) -> jeriya_shared::Result<()> {
        let mut guard = self.shared_backend.immediate_rendering_requests.lock();
        for window_id in self.presenters.keys() {
            let immediate_rendering_request = ImmediateRenderingRequest {
                immediate_command_buffer: AshImmediateCommandBufferHandler {
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

    fn cameras(&self) -> CameraContainerGuard {
        CameraContainerGuard::new(self.shared_backend.camera_event_queue.lock(), self.shared_backend.cameras.lock())
    }

    fn set_active_camera(&self, window_id: WindowId, handle: Handle<Camera>) -> jeriya_shared::Result<()> {
        let presenter = self
            .presenters
            .get(&window_id)
            .ok_or(jeriya_shared::Error::UnknownWindowId(window_id))?;
        presenter.borrow_mut().set_active_camera(handle);
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
