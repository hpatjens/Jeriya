// Allowing doc comments without the Safety section because almost everything is unsafe
// and the Safety section would only repeat the information that can be found in the
// Vulkan specification. Please consider the specification directly when calling
// unsafe functions.
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::missing_safety_doc)]

pub mod buffer;
pub mod command_buffer;
pub mod command_buffer_builder;
pub mod command_pool;
pub mod compute_pipeline;
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
pub mod frame_local_buffer;
pub mod graphics_pipeline;
pub mod host_visible_buffer;
pub mod instance;
pub mod page_buffer;
pub mod physical_device;
pub mod push_descriptors;
pub mod queue;
pub mod queue_plan;
pub mod semaphore;
pub mod shader_interface;
pub mod shader_module;
pub mod specialization_constants;
pub mod staged_push_only_buffer;
pub mod surface;
pub mod swapchain;
pub mod swapchain_depth_buffer;
pub mod swapchain_framebuffers;
pub mod swapchain_render_pass;
pub mod swapchain_vec;
pub mod unsafe_buffer;

use jeriya_content::common::AssetKey;
pub use vk::{DispatchIndirectCommand, DrawIndirectCommand};

use std::{ffi::NulError, str::Utf8Error, sync::Arc};

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

impl<T> AsRawVulkan for Arc<T>
where
    T: AsRawVulkan,
{
    type Output = T::Output;
    fn as_raw_vulkan(&self) -> &Self::Output {
        self.as_ref().as_raw_vulkan()
    }
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
    ShaderInt64,
    MultiDrawIndirect,
    ShaderDrawParameters,
    DrawIndirectCount,
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
    #[error("Failed to find physical devices")]
    NoPhysicalDevices,
    #[error("Failed to find suitable queues")]
    NoSuitableQueues,
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
    #[error("Element was not found")]
    NotFound,
    #[error("Failed to receive asset from asset importer")]
    FailedToReceiveAsset(String), // String contains the details
    #[error("Failed to get asset '{asset_key}' from asset importer: {details}")]
    AssetNotFound { asset_key: AssetKey, details: String },
    #[error("Error from the content module: {:?}", .0)]
    ContentError(#[from] jeriya_content::Error),
}

impl From<Error> for jeriya_backend::Error {
    fn from(value: Error) -> Self {
        jeriya_backend::Error::Backend(Box::new(value))
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
