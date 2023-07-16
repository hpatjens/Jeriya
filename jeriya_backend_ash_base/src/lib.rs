pub mod buffer;
pub mod command_buffer;
pub mod command_buffer_builder;
pub mod command_pool;
pub mod debug;
pub mod descriptor;
pub mod descriptor_pool;
pub mod descriptor_set;
pub mod descriptor_set_layout;
pub mod device;
pub mod device_visible_buffer;
pub mod entry;
pub mod fence;
pub mod frame_index;
pub mod graphics_pipeline;
pub mod host_visible_buffer;
pub mod immediate_graphics_pipeline;
pub mod instance;
pub mod physical_device;
pub mod push_descriptors;
pub mod queue;
pub mod semaphore;
pub mod shader_interface;
pub mod shader_module;
pub mod simple_graphics_pipeline;
pub mod staged_push_only_buffer;
pub mod surface;
pub mod swapchain;
pub mod swapchain_depth_buffer;
pub mod swapchain_framebuffers;
pub mod swapchain_render_pass;
pub mod swapchain_vec;
pub mod unsafe_buffer;

use std::{ffi::NulError, str::Utf8Error};

use ash::{
    extensions::khr::PushDescriptor,
    prelude::VkResult,
    vk::{self},
    LoadingError,
};
use jeriya_shared::{thiserror, winit::window::WindowId, DebugInfo};

pub type Result<T> = std::result::Result<T, Error>;

/// Represents the Vulkan extensions that are used by the backend
pub struct Extensions {
    pub push_descriptor: PushDescriptor,
}

impl Extensions {
    /// Loads the required Extensions
    pub fn new(instance: &ash::Instance, device: &ash::Device) -> Self {
        Self {
            push_descriptor: PushDescriptor::new(instance, device),
        }
    }
}

/// Extension for [`DebugInfo`] to add the memory address of Vulkan handles
pub(crate) trait DebugInfoAshExtension {
    fn with_vulkan_ptr<H>(self, ptr: H) -> Self
    where
        H: vk::Handle;
}

impl DebugInfoAshExtension for DebugInfo {
    fn with_vulkan_ptr<H>(self, ptr: H) -> Self
    where
        H: vk::Handle,
    {
        self.with_ptr(ptr.as_raw())
    }
}

/// Returns the Vulkan equivalent of Self
pub trait AsRawVulkan {
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

#[derive(Debug, Clone)]
pub enum PhysicalDeviceFeature {
    WideLines,
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
    #[error("Failed to find a matching memory type for the memory requirements")]
    UnsupportedMemoryType(vk::MemoryRequirements),
    #[error("Failed to decode SPIR-V code")]
    SpirvDecode,
    #[error("No Pipeline bound")]
    NoPipelineBound,
    #[error("The physical device doesn't support a feature that is expected")]
    PhysicalDeviceFeatureMissing(PhysicalDeviceFeature),
    #[error("The descriptor pool doesn't have enough space")]
    DescriptorPoolDoesntHaveEnoughSpace,
    #[error("Failed to allocate the resource")]
    FailedToAllocate(&'static str),
    #[error("BufferOverflow")]
    WouldOverflow,
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
    pub validation_layer: ValidationLayerConfig,
}
