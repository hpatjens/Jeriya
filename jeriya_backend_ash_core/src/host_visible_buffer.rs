use std::{mem, sync::Arc};

use ash::vk;
use jeriya_shared::{AsDebugInfo, DebugInfo};

use crate::{buffer::BufferUsageFlags, command_buffer::CommandBufferDependency, device::Device, unsafe_buffer::UnsafeBuffer, AsRawVulkan};

pub struct HostVisibleBuffer<T> {
    buffer: UnsafeBuffer<T>,
    len: usize,
}

impl<T: Copy> HostVisibleBuffer<T> {
    /// Creates a new [`HostVisibleBuffer`] with the given data and usage flags
    pub fn new(device: &Arc<Device>, data: &[T], usage: BufferUsageFlags, debug_info: DebugInfo) -> crate::Result<Arc<Self>> {
        assert!(data.len() > 0, "HostVisibleBuffer must have a non-zero size");
        let buffer = unsafe {
            let size = mem::size_of_val(data);
            let mut buffer = UnsafeBuffer::new(device, size, usage.into(), vk::SharingMode::EXCLUSIVE, debug_info)?;
            buffer.allocate_memory(vk::MemoryPropertyFlags::HOST_VISIBLE)?;
            buffer.set_memory_unaligned(data)?;
            buffer
        };
        Ok(Arc::new(Self { buffer, len: data.len() }))
    }

    /// Returns the underlying [`UnsafeBuffer`]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the size of the buffer in bytes
    pub fn byte_size(&self) -> usize {
        self.buffer.byte_size()
    }
}

impl<T> AsRawVulkan for HostVisibleBuffer<T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.buffer.as_raw_vulkan()
    }
}

impl<T> AsDebugInfo for HostVisibleBuffer<T> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.buffer.as_debug_info()
    }
}

impl<T> CommandBufferDependency for HostVisibleBuffer<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    mod new {
        use jeriya_shared::debug_info;

        use crate::{buffer::BufferUsageFlags, device::tests::TestFixtureDevice};

        use super::HostVisibleBuffer;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let _buffer = HostVisibleBuffer::<f32>::new(
                &device_test_fixture.device,
                &[1.0, 2.0, 3.0],
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("my_host_visible_buffer"),
            )
            .unwrap();
        }
    }
}
