use std::sync::Arc;

use ash::vk;
use jeriya_shared::bitflags::bitflags;

use crate::{
    command_buffer::CommandBufferDependency, device_visible_buffer::DeviceVisibleBuffer, host_visible_buffer::HostVisibleBuffer,
    AsRawVulkan,
};

bitflags! {
    /// Flags that specify the usage of a buffer
    pub struct BufferUsageFlags: u32 {
        // WARNING: Has to match the Vulkan flags by value
        // https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkBufferUsageFlagBits.html
        const TRANSFER_SRC_BIT = 0x00000001;
        const TRANSFER_DST_BIT = 0x00000002;
        const UNIFORM_BUFFER = 0x00000010;
        const STORAGE_BUFFER = 0x00000020;
        const VERTEX_BUFFER = 0x00000080;
        const INDIRECT_BUFFER = 0x00000100;
    }
}

impl From<BufferUsageFlags> for vk::BufferUsageFlags {
    fn from(flags: BufferUsageFlags) -> Self {
        vk::BufferUsageFlags::from_raw(flags.bits())
    }
}

pub trait Buffer<T>: AsRawVulkan<Output = vk::Buffer> {}

impl<E, T> Buffer<T> for Arc<E> where E: Buffer<T> {}

/// A buffer that can be used as a vertex buffer
///
/// #Notes
///
/// This exists as a workaround for dyn upcasting coercion (https://github.com/rust-lang/rust/issues/65991).
/// When passing a [`HostVisibleBuffer`] or [`DeviceVisibleBuffer`] to a function that wants to append the
/// buffer to a [`CommandBuffer`], it has to be usable as a `dyn CommandBufferDependency`. But it's currently
/// not possible to cast an `Arc<dyn Buffer>` with `trait Buffer: CommandBufferDependency` to an
/// `Arc<dyn CommandBufferDependency>`.
pub enum VertexBuffer<'arc, T> {
    HostVisibleBuffer(&'arc Arc<HostVisibleBuffer<T>>),
    DeviceVisibleBuffer(&'arc Arc<DeviceVisibleBuffer<T>>),
}

impl<'arc, T: 'static> VertexBuffer<'arc, T> {
    pub fn as_command_buffer_dependency(&self) -> Arc<dyn CommandBufferDependency> {
        match self {
            Self::HostVisibleBuffer(buffer) => (*buffer).clone(),
            Self::DeviceVisibleBuffer(buffer) => (*buffer).clone(),
        }
    }
}

impl<'arc, T> AsRawVulkan for VertexBuffer<'arc, T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        match self {
            Self::HostVisibleBuffer(buffer) => buffer.as_raw_vulkan(),
            Self::DeviceVisibleBuffer(buffer) => buffer.as_raw_vulkan(),
        }
    }
}

impl<'arc, T> From<&'arc Arc<HostVisibleBuffer<T>>> for VertexBuffer<'arc, T> {
    fn from(buffer: &'arc Arc<HostVisibleBuffer<T>>) -> Self {
        Self::HostVisibleBuffer(buffer)
    }
}

impl<'arc, T> From<&'arc Arc<DeviceVisibleBuffer<T>>> for VertexBuffer<'arc, T> {
    fn from(buffer: &'arc Arc<DeviceVisibleBuffer<T>>) -> Self {
        Self::DeviceVisibleBuffer(buffer)
    }
}
