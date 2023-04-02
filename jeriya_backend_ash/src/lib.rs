mod ash_backend;
mod debug;
mod device;
mod entry;
mod instance;
mod physical_device;
mod queue;
mod surface;
mod swapchain;

pub use ash_backend::*;

use std::{ffi::NulError, str::Utf8Error};

use ash::{
    prelude::VkResult,
    vk::{self},
    LoadingError,
};
use jeriya_shared::winit::window::WindowId;

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
        self.map_err(Error::Result)
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
    #[error("Failed to find the WindowId: {:?}", .0)]
    UnknownWindowId(WindowId),
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
