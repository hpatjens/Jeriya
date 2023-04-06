use std::cell::RefCell;
use std::rc::Rc;
use std::{collections::HashMap, sync::Arc};

use jeriya::Backend;
use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    command_pool::CommandPool,
    command_pool::CommandPoolCreateFlags,
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    instance::Instance,
    physical_device::PhysicalDevice,
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
    _presenters: HashMap<WindowId, RefCell<Presenter>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    _device: Arc<Device>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    _command_pool: Rc<CommandPool>,
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
        let command_pool = CommandPool::new(&device, &device.presentation_queue, CommandPoolCreateFlags::ResetCommandBuffer)?;

        Ok(Self {
            _device: device,
            _validation_layer_callback: validation_layer_callback,
            _entry: entry,
            _instance: instance,
            _presenters: presenters,
            _surfaces: surfaces,
            _command_pool: command_pool,
        })
    }

    fn handle_window_resized(&self, window_id: WindowId) -> jeriya_shared::Result<()> {
        let mut presenter = self
            ._presenters
            .get(&window_id)
            .ok_or_else(|| core::Error::UnknownWindowId(window_id))?
            .borrow_mut();
        presenter.recreate()?;
        Ok(())
    }

    fn handle_render_frame(&self) -> jeriya_shared::Result<()> {
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
