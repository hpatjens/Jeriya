use ash::vk;
use jeriya_shared::{AsDebugInfo, DebugInfo};

use std::{marker::PhantomData, mem, slice, sync::Arc};

use crate::{device::Device, AsRawVulkan, DebugInfoAshExtension, Error};

/// Buffer implementation that is used by [`DeviceVisibleBuffer`] and [`HostVisibleBuffer`]
pub struct UnsafeBuffer<T> {
    device: Arc<Device>,
    buffer: vk::Buffer,
    buffer_memory: Option<vk::DeviceMemory>,
    byte_size: usize,
    phantom_data: PhantomData<T>,
    debug_info: DebugInfo,
}

impl<T: Clone> UnsafeBuffer<T> {
    /// Creates a new buffer with the given size and usage
    pub unsafe fn new(
        device: &Arc<Device>,
        byte_size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        debug_info: DebugInfo,
    ) -> crate::Result<Self> {
        assert!(byte_size > 0, "UnsafeBuffer must have a non-zero size");
        let buffer_create_info = vk::BufferCreateInfo::builder()
            .size(byte_size as u64)
            .usage(usage)
            .sharing_mode(sharing_mode)
            .queue_family_indices(&device.queue_plan.queue_family_indices);
        let buffer = device.as_raw_vulkan().create_buffer(&buffer_create_info, None)?;
        let debug_info = debug_info.with_vulkan_ptr(buffer);
        Ok(Self {
            device: device.clone(),
            buffer,
            buffer_memory: None,
            byte_size,
            phantom_data: PhantomData,
            debug_info,
        })
    }

    /// Allocates memory for the buffer and binds it to the buffer
    pub unsafe fn allocate_memory(&mut self, memory_properties: vk::MemoryPropertyFlags) -> crate::Result<()> {
        assert_eq!(self.buffer_memory, None, "allocate_memory must only be called once");
        let device = self.device.as_raw_vulkan();
        let memory_requirements = device.get_buffer_memory_requirements(self.buffer);
        let memory_type_index = self
            .device
            .find_memorytype_index(&memory_requirements, memory_properties)
            .ok_or(Error::UnsupportedMemoryType(memory_requirements))?;
        let vertex_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: memory_requirements.size,
            memory_type_index,
            ..Default::default()
        };
        let buffer_memory = device.allocate_memory(&vertex_buffer_allocate_info, None)?;
        device.bind_buffer_memory(self.buffer, buffer_memory, 0)?;
        self.buffer_memory = Some(buffer_memory);
        Ok(())
    }

    /// Maps the GPU memory of the buffer and returns a slice to it
    unsafe fn map_buffer_memory(&mut self) -> crate::Result<(&mut [T], vk::DeviceMemory)> {
        assert!(self.buffer_memory.is_some(), "allocate_memory must be called before set_memory");
        let buffer_memory = self
            .buffer_memory
            .expect("buffer_memory must be initialized when set_memory is called");
        let ptr = self
            .device
            .as_raw_vulkan()
            .map_memory(buffer_memory, 0, self.byte_size as u64, vk::MemoryMapFlags::empty())?;
        let slice = slice::from_raw_parts_mut(ptr as *mut T, self.byte_size / mem::size_of::<T>());
        Ok((slice, buffer_memory))
    }

    /// Copies the given `data` into the buffer
    ///
    /// # Panics
    ///
    /// Panics if the `data` does not fit into the buffer exactly
    pub unsafe fn set_memory_unaligned(&mut self, data: &[T]) -> crate::Result<()> {
        assert_eq!(
            self.byte_size,
            mem::size_of_val(data),
            "the data has to fit into the buffer exactly"
        );
        let (slice, buffer_memory) = self.map_buffer_memory()?;
        slice.clone_from_slice(data);
        self.device.as_raw_vulkan().unmap_memory(buffer_memory);
        Ok(())
    }

    /// Copies the given `data` into the buffer at the given `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub unsafe fn set_memory_unaligned_index(&mut self, index: usize, value: &T) -> crate::Result<()> {
        let size = mem::size_of_val::<T>(value);
        let offset = index * mem::size_of::<T>();
        assert!(
            self.byte_size > offset + size,
            "the data doesn't fit into the buffer at the given offset"
        );
        let (slice, buffer_memory) = self.map_buffer_memory()?;
        slice[index..index + 1].clone_from_slice(slice::from_ref(value));
        self.device.as_raw_vulkan().unmap_memory(buffer_memory);
        Ok(())
    }

    /// Copies the `data` from the buffer into the given slice
    ///
    /// # Panics
    ///
    /// Panics if the `data` does not have the same size as the buffer
    pub unsafe fn get_memory_unaligned(&mut self, data: &mut [T]) -> crate::Result<()> {
        assert_eq!(self.byte_size, mem::size_of_val(data), "data must have the same size as the buffer");
        let (slice, buffer_memory) = self.map_buffer_memory()?;
        data.clone_from_slice(slice);
        self.device.as_raw_vulkan().unmap_memory(buffer_memory);
        Ok(())
    }

    /// Returns the size of the buffer in bytes
    pub fn byte_size(&self) -> usize {
        self.byte_size
    }
}

impl<T> Drop for UnsafeBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            let device = self.device.as_raw_vulkan();
            if let Some(buffer_memory) = self.buffer_memory {
                device.free_memory(buffer_memory, None);
            }
            device.destroy_buffer(self.buffer, None);
        }
    }
}

impl<T> AsRawVulkan for UnsafeBuffer<T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.buffer
    }
}

impl<T> AsDebugInfo for UnsafeBuffer<T> {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod new {
        use ash::vk;
        use jeriya_shared::debug_info;

        use crate::device::tests::TestFixtureDevice;

        use super::UnsafeBuffer;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let data = vec![1.0f32, 2.0, 3.0, 4.0];
            unsafe {
                let mut buffer = UnsafeBuffer::<f32>::new(
                    &device_test_fixture.device,
                    std::mem::size_of_val(data.as_slice()),
                    vk::BufferUsageFlags::TRANSFER_SRC,
                    vk::SharingMode::EXCLUSIVE,
                    debug_info!("my_unsafe_buffer"),
                )
                .unwrap();
                assert_eq!(buffer.byte_size(), 16);
                buffer
                    .allocate_memory(vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                    .unwrap();
                buffer.set_memory_unaligned(&data).unwrap();
                let mut data2 = vec![0.0; 4];
                buffer.get_memory_unaligned(&mut data2).unwrap();
                assert_eq!(data, data2);
            }
        }

        #[test]
        fn set_memory_unaligned_index() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let data = vec![1.0f32, 2.0, 3.0, 4.0];
            unsafe {
                let mut buffer = UnsafeBuffer::<f32>::new(
                    &device_test_fixture.device,
                    std::mem::size_of_val(data.as_slice()),
                    vk::BufferUsageFlags::TRANSFER_SRC,
                    vk::SharingMode::EXCLUSIVE,
                    debug_info!("my_unsafe_buffer"),
                )
                .unwrap();
                assert_eq!(buffer.byte_size(), 16);
                buffer
                    .allocate_memory(vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
                    .unwrap();
                buffer.set_memory_unaligned(&data).unwrap();
                // Replace the 3 with a 6
                buffer.set_memory_unaligned_index(2, &6.0).unwrap();
                let mut data2 = vec![0.0; 4];
                buffer.get_memory_unaligned(&mut data2).unwrap();
                assert_eq!(vec![1.0, 2.0, 6.0, 4.0], data2);
            }
        }
    }
}
