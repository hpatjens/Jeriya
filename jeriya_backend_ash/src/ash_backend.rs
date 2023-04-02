use std::{collections::HashMap, sync::Arc};

use jeriya::Backend;

use jeriya_shared::{
    log::info,
    winit::window::{Window, WindowId},
    RendererConfig,
};

use crate::{
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    instance::Instance,
    physical_device::PhysicalDevice,
    surface::Surface,
    swapchain::Swapchain,
    Config, ValidationLayerConfig,
};

pub struct Ash {
    _swapchains: HashMap<WindowId, Swapchain>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    _device: Arc<Device>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
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
                let surface = Surface::new(&entry, &instance, window)?;
                Ok((*window_id, surface))
            })
            .collect::<crate::Result<HashMap<WindowId, Arc<Surface>>>>()?;

        // Device
        let physical_device = PhysicalDevice::new(&instance, surfaces.values())?;
        let device = Device::new(physical_device, &instance)?;

        // Swapchains
        let swapchains = surfaces
            .iter()
            .map(|(window_id, surface)| {
                let swapchain = Swapchain::new(&instance, &device, surface)?;
                Ok((*window_id, swapchain))
            })
            .collect::<crate::Result<HashMap<WindowId, Swapchain>>>()?;

        Ok(Self {
            _device: device,
            _validation_layer_callback: validation_layer_callback,
            _entry: entry,
            _instance: instance,
            _swapchains: swapchains,
            _surfaces: surfaces,
        })
    }

    fn handle_window_resized(&self, window_id: WindowId) -> jeriya_shared::Result<()> {
        let swapchain = self
            ._swapchains
            .get(&window_id)
            .ok_or_else(|| crate::Error::UnknownWindowId(window_id))?;
        swapchain.recreate()?;
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
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            Ash::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            let renderer_config = RendererConfig { application_name: None };
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
