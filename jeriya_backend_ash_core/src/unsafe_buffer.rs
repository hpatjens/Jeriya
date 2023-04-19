use ash::{util::Align, vk};

use std::{
    marker::PhantomData,
    mem::{self, align_of},
    sync::Arc,
};

use crate::{device::Device, AsRawVulkan, Error};

/// Buffer implementation that is used by [`DeviceVisibleBuffer`] and [`HostVisibleBuffer`]
pub struct UnsafeBuffer<T> {
    device: Arc<Device>,
    buffer: vk::Buffer,
    buffer_memory: Option<vk::DeviceMemory>,
    size: usize,
    phantom_data: PhantomData<T>,
}

impl<T: Copy> UnsafeBuffer<T> {
    /// Creates a new buffer with the given size and usage
    pub unsafe fn new(
        device: &Arc<Device>,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
    ) -> crate::Result<Self> {
        assert!(size > 0, "UnsafeBuffer must have a non-zero size");
        let buffer_create_info = vk::BufferCreateInfo {
            size: size as u64,
            usage,
            sharing_mode,
            ..Default::default()
        };
        let buffer = device.as_raw_vulkan().create_buffer(&buffer_create_info, None)?;
        Ok(Self {
            device: device.clone(),
            buffer,
            buffer_memory: None,
            size,
            phantom_data: PhantomData,
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

    /// Copies the given data into the buffer
    pub unsafe fn set_memory(&mut self, data: &[T]) -> crate::Result<()> {
        assert!(self.buffer_memory.is_some(), "allocate_memory must be called before set_memory");
        assert_eq!(self.size, mem::size_of_val(data), "the data has to fit into the buffer exactly");
        let buffer_memory = self
            .buffer_memory
            .expect("buffer_memory must be initialized when set_memory is called");
        let device = self.device.as_raw_vulkan();
        let ptr = device.map_memory(buffer_memory, 0, self.size as u64, vk::MemoryMapFlags::empty())?;
        let mut align = Align::new(ptr, align_of::<T>() as u64, self.size as u64);
        align.copy_from_slice(&data);
        device.unmap_memory(buffer_memory);
        Ok(())
    }

    /// Returns the size of the buffer
    pub fn size(&self) -> usize {
        self.size
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

#[cfg(test)]
mod tests {
    use super::*;

    mod new {
        use crate::device::tests::TestFixtureDevice;

        use super::UnsafeBuffer;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let _buffer = unsafe {
                UnsafeBuffer::<f32>::new(
                    &device_test_fixture.device,
                    1024,
                    ash::vk::BufferUsageFlags::TRANSFER_SRC,
                    ash::vk::SharingMode::EXCLUSIVE,
                )
                .unwrap()
            };
        }
    }
}
