use std::sync::{mpsc::Receiver, Arc};

use ash::vk;
use jeriya_shared::{debug_info, DebugInfo};

use crate::{
    buffer::BufferUsageFlags, command_buffer_builder::CommandBufferBuilder, device::Device, device_visible_buffer::DeviceVisibleBuffer,
    host_visible_buffer::HostVisibleBuffer, AsRawVulkan, Error,
};

/// Buffer that stores pages of data
pub struct PageBuffer<P> {
    device_visible_buffer: Arc<DeviceVisibleBuffer<P>>,
    device: Arc<Device>,
    capacity: usize,
    debug_info: DebugInfo,

    page_table: Vec<bool>,
    free_list: Vec<usize>,

    /// Number of pages that are currently in use
    len: usize,
}

impl<P: Clone + Send + Sync + 'static> PageBuffer<P> {
    /// Creates a new [`PageBuffer`] with the given `capacity` and `buffer_usage_flags`. Capacity is not measured in bytes but in the number of pages of type `P`.
    pub fn new(device: &Arc<Device>, capacity: usize, buffer_usage_flags: BufferUsageFlags, debug_info: DebugInfo) -> crate::Result<Self> {
        let device_visible_buffer =
            DeviceVisibleBuffer::new(device, capacity * std::mem::size_of::<P>(), buffer_usage_flags, debug_info.clone())?;
        Ok(Self {
            device_visible_buffer,
            device: device.clone(),
            capacity,
            debug_info,
            page_table: vec![false; capacity],
            free_list: (0..capacity).collect(),
            len: 0,
        })
    }

    /// Inserts the given pages into the buffer and returns the indices of the inserted pages
    ///
    /// If the length of the given `pages` is greater than the number of free pages in the buffer, an error is returned.
    pub fn insert(&mut self, pages: Vec<P>, command_buffer_builder: &mut CommandBufferBuilder) -> crate::Result<Vec<usize>> {
        if pages.len() > self.free_pages() {
            return Err(Error::WouldOverflow);
        }

        // Find free pages
        let indices = self.free_list.iter().take(pages.len()).copied().collect::<Vec<_>>();
        jeriya_shared::assert_eq!(indices.len(), pages.len(), "Allocated indices and pages must have the same length");

        for i in 0..pages.len() {
            self.page_table[indices[i]] = true;
            self.free_list.remove(0);
        }
        self.len += pages.len();
        jeriya_shared::assert!(self.len <= self.capacity, "len must not exceed capacity");

        // Create staging buffer
        let byte_size = pages.len() * std::mem::size_of::<P>();
        let host_visible_buffer = Arc::new(HostVisibleBuffer::<P>::new(
            &self.device,
            pages.as_slice(),
            BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!("StagingBuffer-for-PageBuffer"),
        )?);

        // Create the commands to transfer the individual pages from the staging buffer to the device visible buffer
        for i in 0..pages.len() {
            let page_index = indices[i];
            let src_offset = i * std::mem::size_of::<P>();
            let dst_offset = page_index * std::mem::size_of::<P>();

            unsafe {
                let copy_region = vk::BufferCopy {
                    src_offset: src_offset as u64,
                    dst_offset: dst_offset as u64,
                    size: byte_size as u64,
                };
                let copy_regions = [copy_region];
                let command_buffer = command_buffer_builder.command_buffer();
                self.device.as_raw_vulkan().cmd_copy_buffer(
                    *command_buffer.as_raw_vulkan(),
                    *host_visible_buffer.as_raw_vulkan(),
                    *self.device_visible_buffer.as_raw_vulkan(),
                    &copy_regions,
                );
                command_buffer.push_dependency(host_visible_buffer.clone());
                command_buffer.push_dependency(self.device_visible_buffer.clone());
            }
        }

        Ok(indices)
    }

    /// Sets the given pages to status unoccupied but doesn't touch the actual data in the buffer
    ///
    /// Returns the number of pages that were freed because indices to non-occupied pages are ignored.
    ///
    /// # Panics
    ///
    /// * If the given indices are out of bounds
    pub fn free(&mut self, indices: &[usize]) -> crate::Result<usize> {
        jeriya_shared::assert!(indices.len() <= self.len, "Cannot free more pages than are currently in use");
        let mut count = 0;
        for i in indices {
            if *i > self.capacity {
                return Err(Error::NotFound);
            }

            // Only free pages that are actually occupied
            if self.page_table[*i] {
                self.page_table[*i] = false;
                self.free_list.push(*i);
                count += 1;
            }
        }
        self.len -= count;

        Ok(count)
    }

    /// Returns the overall number of pages that can be inserted into the buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of free pages. This is the same as `capacity - len`.
    pub fn free_pages(&self) -> usize {
        self.capacity - self.len
    }

    /// Returns the number of occupied pages
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the `DebugInfo` of the buffer
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl<P: Clone + Send + Sync + Default + 'static> PageBuffer<P> {
    pub fn read_all(&mut self, command_buffer_builder: &mut CommandBufferBuilder) -> crate::Result<Receiver<Vec<P>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::debug_info;

    use crate::{
        command_buffer::{tests::TestFixtureCommandBuffer, CommandBuffer},
        device::TestFixtureDevice,
    };

    use super::*;

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    struct Page {
        data: [i32; 4],
    }

    impl Page {
        pub fn one() -> Self {
            Self { data: [1; 4] }
        }

        pub fn two() -> Self {
            Self { data: [2; 4] }
        }
    }

    #[test]
    fn empty() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let buffer = PageBuffer::<f32>::new(
            &test_fixture_device.device,
            5,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("my_host_visible_buffer"),
        )
        .unwrap();

        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), 5);
        assert_eq!(buffer.free_pages(), 5);
        assert!(buffer.is_empty());
    }

    #[test]
    fn insert() {
        // Fixtures
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();

        // Create buffer
        let mut buffer = PageBuffer::<Page>::new(
            &test_fixture_device.device,
            5,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("my_host_visible_buffer"),
        )
        .unwrap();

        // Create CommandBuffer
        let mut command_buffer_builder =
            CommandBufferBuilder::new(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();
        command_buffer_builder.begin_command_buffer().unwrap();

        // Insert pages which are just floats for the test
        let indices = buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();

        // Assertions
        assert_eq!(indices.len(), 2);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 1);

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.free_pages(), 3);
        assert!(!buffer.is_empty());

        command_buffer_builder.end_command_buffer().unwrap();

        // Submit
        test_fixture_command_buffer
            .queue
            .submit(test_fixture_command_buffer.command_buffer)
            .unwrap();
        test_fixture_device.device.wait_for_idle().unwrap();
    }

    #[test]
    fn free() {
        // Fixtures
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();

        // Create buffer
        let mut buffer = PageBuffer::<Page>::new(
            &test_fixture_device.device,
            5,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("my_host_visible_buffer"),
        )
        .unwrap();

        // Create CommandBuffer
        let mut command_buffer_builder =
            CommandBufferBuilder::new(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();
        command_buffer_builder.begin_command_buffer().unwrap();

        // Insert pages which are just floats for the test
        let indices = buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();

        command_buffer_builder.end_command_buffer().unwrap();

        // Free pages
        let count = buffer.free(&indices).unwrap();

        // Assertions
        assert_eq!(count, 2);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.free_pages(), 5);
        assert!(buffer.is_empty());
    }

    // #[test]
    // fn read_memory() {
    //     // Fixtures
    //     let test_fixture_device = TestFixtureDevice::new().unwrap();
    //     let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();

    //     // Create buffer
    //     let mut buffer = PageBuffer::<Page>::new(
    //         &test_fixture_device.device,
    //         5,
    //         BufferUsageFlags::STORAGE_BUFFER,
    //         debug_info!("my_host_visible_buffer"),
    //     )
    //     .unwrap();

    //     // Command Buffer 1
    //     let mut command_buffer_builder =
    //         CommandBufferBuilder::new(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();
    //     command_buffer_builder.begin_command_buffer().unwrap();
    //     buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();
    //     command_buffer_builder.end_command_buffer().unwrap();

    //     // Submit
    //     test_fixture_command_buffer
    //         .queue
    //         .submit(test_fixture_command_buffer.command_buffer)
    //         .unwrap();
    //     test_fixture_device.device.wait_for_idle().unwrap();

    //     // Command Buffer 2
    //     let mut command_buffer = CommandBuffer::new(
    //         &test_fixture_device.device,
    //         &test_fixture_command_buffer.command_pool,
    //         debug_info!("my_command_buffer"),
    //     )
    //     .unwrap();
    //     let mut command_buffer_builder = CommandBufferBuilder::new(&test_fixture_device.device, &mut command_buffer).unwrap();
    //     command_buffer_builder.begin_command_buffer().unwrap();
    //     buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();
    //     command_buffer_builder.end_command_buffer().unwrap();

    //     // Read memory back
    //     command_buffer_builder.begin_command_buffer().unwrap();
    //     let receiver = buffer.read_all(&mut command_buffer_builder).unwrap();
    //     command_buffer_builder.end_command_buffer().unwrap();

    //     // Submit
    //     test_fixture_command_buffer.queue.submit(command_buffer).unwrap();

    //     // Wait for GPU
    //     test_fixture_device.device.wait_for_idle().unwrap();
    //     test_fixture_command_buffer.queue.poll_completed_fences().unwrap();

    //     // Check result
    //     let pages = receiver.recv().unwrap();
    //     assert_eq!(pages.len(), 2);
    //     assert_eq!(pages[0], Page::one());
    //     assert_eq!(pages[1], Page::two());
    // }
}
