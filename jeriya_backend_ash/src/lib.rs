mod debug;
mod device;
mod entry;
mod instance;
mod physical_device;
mod queue;
mod surface;
mod swapchain;

use std::{ffi::NulError, str::Utf8Error, sync::Arc};

use debug::ValidationLayerCallback;
use instance::Instance;
use jeriya::Backend;

use ash::{
    prelude::VkResult,
    vk::{self},
    LoadingError,
};
use jeriya_shared::{log::info, winit::window::Window, RendererConfig};

use crate::{debug::set_panic_on_message, device::Device, entry::Entry, physical_device::PhysicalDevice, surface::Surface};

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) trait AsRawVulkan {
    type Output;
    fn as_raw_vulkan(&self) -> &Self::Output;
}

pub(crate) trait IntoJeriya {
    type Output;
    fn into_jeriya(self) -> Self::Output;
}

impl<T> IntoJeriya for VkResult<T> {
    type Output = Result<T>;

    fn into_jeriya(self) -> Self::Output {
        self.map_err(|err| Error::Result(err))
    }
}

/// Errors in the ash backend
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error while loading Vulkan {:?}", .0)]
    LoadingError(#[from] LoadingError),
    #[error("Wrong platform")]
    WrongPlatform,
    #[error("Error while executing a Vulkan operation {:?}", .0)]
    Result(#[from] vk::Result),
    #[error("Error while converting a string: {:?}", .0)]
    StringNulError(#[from] NulError),
    #[error("Error while converting a string to UTF-8: {:?}", .0)]
    StringUtf8Error(#[from] Utf8Error),
    #[error("Error concerning physical device: {:?}", .0)]
    PhysicalDeviceError(#[from] physical_device::Error),
    #[error("Failed to find a suitable swapchain surface format")]
    SwapchainSurfaceFormatError,
}

impl From<Error> for jeriya_shared::Error {
    fn from(value: Error) -> Self {
        jeriya_shared::Error::Backend(Box::new(value))
    }
}

pub enum ValidationLayerConfig {
    Disabled,
    Enabled { panic_on_message: bool },
}

impl Default for ValidationLayerConfig {
    fn default() -> Self {
        Self::Enabled { panic_on_message: true }
    }
}

#[derive(Default)]
pub struct Config {
    validation_layer: ValidationLayerConfig,
}

pub struct Ash {
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
        let surfaces = windows
            .iter()
            .map(|window| Surface::new(&entry, &instance, &window))
            .collect::<Result<Vec<Surface>>>()?;

        let physical_device = PhysicalDevice::new(&instance, &surfaces)?;

        let device = Device::new(physical_device, &instance)?;

        Ok(Self {
            _device: device,
            _validation_layer_callback: validation_layer_callback,
            _entry: entry,
            _instance: instance,
        })
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
