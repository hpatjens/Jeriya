use std::{mem, sync::Arc};

use ash::vk;
use jeriya_shared::{parking_lot::Mutex, AsDebugInfo, DebugInfo};

use crate::{
    buffer::{Buffer, BufferUsageFlags, GeneralBuffer},
    command_buffer::CommandBufferDependency,
    device::Device,
    unsafe_buffer::UnsafeBuffer,
    AsRawVulkan,
};

pub struct HostVisibleBuffer<T> {
    buffer: UnsafeBuffer<T>,
    len: usize,
}

impl<T: Clone> HostVisibleBuffer<T> {
    /// Creates a new [`HostVisibleBuffer`] with the given data and usage flags
    pub fn new(device: &Arc<Device>, data: &[T], usage: BufferUsageFlags, debug_info: DebugInfo) -> crate::Result<Self> {
        assert!(!data.is_empty(), "HostVisibleBuffer must have a non-zero size");
        let buffer = unsafe {
            let size = mem::size_of_val(data);
            let mut buffer = UnsafeBuffer::new(device, size, usage.into(), vk::SharingMode::CONCURRENT, debug_info)?;
            buffer.allocate_memory(vk::MemoryPropertyFlags::HOST_VISIBLE)?;
            buffer.set_memory_unaligned(data)?;
            buffer
        };
        Ok(Self { buffer, len: data.len() })
    }

    /// Writes the given data to the buffer
    pub fn set_memory_unaligned(&mut self, data: &[T]) -> crate::Result<()> {
        unsafe {
            self.buffer.set_memory_unaligned(data)?;
        }
        Ok(())
    }

    /// Writes the given data to the buffer at the given index
    pub fn set_memory_unaligned_index(&mut self, index: usize, data: &T) -> crate::Result<()> {
        unsafe {
            self.buffer.set_memory_unaligned_index(index, data)?;
        }
        Ok(())
    }

    /// Reads the buffer into the given slice
    pub fn get_memory_unaligned(&self, data: &mut [T]) -> crate::Result<()> {
        unsafe {
            self.buffer.get_memory_unaligned(data)?;
        }
        Ok(())
    }

    /// Returns the underlying [`UnsafeBuffer`]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the size of the buffer in bytes
    pub fn byte_size(&self) -> usize {
        self.buffer.byte_size()
    }
}

impl<T> GeneralBuffer for HostVisibleBuffer<T> {}
impl<T> Buffer<T> for HostVisibleBuffer<T> {}

impl<T> AsRawVulkan for HostVisibleBuffer<T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        self.buffer.as_raw_vulkan()
    }
}

impl<T> AsDebugInfo for HostVisibleBuffer<T> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.buffer.as_debug_info()
    }
}

impl<T: Send + Sync> CommandBufferDependency for Mutex<HostVisibleBuffer<T>> {}

#[cfg(test)]
mod tests {
    use super::*;

    mod new {
        use jeriya_shared::debug_info;

        use crate::{buffer::BufferUsageFlags, device::TestFixtureDevice};

        use super::HostVisibleBuffer;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let data = [1.0, 2.0, 3.0];
            let buffer = HostVisibleBuffer::<f32>::new(
                &device_test_fixture.device,
                &data,
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("my_host_visible_buffer"),
            )
            .unwrap();
            let mut data2 = [0.0, 0.0, 0.0];
            buffer.get_memory_unaligned(&mut data2).unwrap();
            assert_eq!(data, data2);
        }

        #[test]
        fn set_memory() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let zeroed = [0.0, 0.0, 0.0];
            let mut buffer = HostVisibleBuffer::<f32>::new(
                &device_test_fixture.device,
                &zeroed,
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("my_host_visible_buffer"),
            )
            .unwrap();
            let data = [1.0, 2.0, 3.0];
            buffer.set_memory_unaligned(&data).unwrap();
            let mut data2 = [0.0, 0.0, 0.0];
            buffer.get_memory_unaligned(&mut data2).unwrap();
            assert_eq!(data, data2);
        }
    }
}
