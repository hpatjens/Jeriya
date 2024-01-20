use std::sync::{mpsc::Receiver, Arc};

use ash::vk;
use jeriya_shared::{debug_info, parking_lot::Mutex, DebugInfo};

use crate::{
    buffer::{Buffer, BufferUsageFlags, GeneralBuffer},
    command_buffer_builder::CommandBufferBuilder,
    device::Device,
    device_visible_buffer::DeviceVisibleBuffer,
    host_visible_buffer::HostVisibleBuffer,
    AsRawVulkan, Error,
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
        let device_visible_buffer = DeviceVisibleBuffer::new(
            device,
            capacity * std::mem::size_of::<P>(),
            buffer_usage_flags | BufferUsageFlags::TRANSFER_DST_BIT | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info.clone(),
        )?;
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
        let host_visible_buffer = Arc::new(HostVisibleBuffer::<P>::new(
            &self.device,
            pages.as_slice(),
            BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!("StagingBuffer-for-PageBuffer"),
        )?);

        // Create the commands to transfer the individual pages from the staging buffer to the device visible buffer
        for (i, page_index) in indices.iter().enumerate().take(pages.len()) {
            let src_offset = i * std::mem::size_of::<P>();
            let dst_offset = page_index * std::mem::size_of::<P>();

            unsafe {
                let copy_region = vk::BufferCopy {
                    src_offset: src_offset as u64,
                    dst_offset: dst_offset as u64,
                    size: std::mem::size_of::<P>() as u64,
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
    /// Reads all pages from the buffer and returns them in a [`Receiver`]
    ///
    /// This should only be used for debugging purposes since it is very slow.
    pub fn read_all(&mut self, command_buffer_builder: &mut CommandBufferBuilder) -> crate::Result<Receiver<Vec<P>>> {
        let host_visible_buffer = Arc::new(Mutex::new(HostVisibleBuffer::<P>::new(
            &self.device,
            &vec![Default::default(); self.capacity],
            BufferUsageFlags::TRANSFER_DST_BIT,
            debug_info!("HostVisibleBuffer-for-PageBuffer"),
        )?));
        let byte_size = self.capacity * std::mem::size_of::<P>();
        command_buffer_builder.copy_buffer_range_from_device_to_host(&self.device_visible_buffer, 0, &host_visible_buffer, 0, byte_size);

        // Since there might be holes in the page table, we need to filter out the indices that are actually occupied.
        let indices = self
            .page_table
            .iter()
            .enumerate()
            .filter(|(_, occupied)| **occupied)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();

        // Enqueue finished operation to get the data from the host visible buffer.
        let len = self.capacity;
        let (sender, receiver) = std::sync::mpsc::channel();
        command_buffer_builder.push_finished_operation(Box::new(move || {
            // Copy the data from the host visible buffer into a temporary buffer
            let mut temp_data = vec![Default::default(); len];
            let host_visible_buffer = host_visible_buffer.lock();
            host_visible_buffer.get_memory_unaligned(&mut temp_data)?;

            // Copy only the occupied pages into the final buffer
            let mut data = vec![Default::default(); len];
            for index in &indices {
                data[*index] = temp_data[*index].clone();
            }

            sender
                .send(temp_data)
                .expect("Failed to send data from PageBuffer to receiver in finished operation.");
            Ok(())
        }));
        Ok(receiver)
    }
}

impl<P> AsRawVulkan for PageBuffer<P> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        self.device_visible_buffer.as_raw_vulkan()
    }
}

impl<T> GeneralBuffer for PageBuffer<T> {}
impl<T> Buffer<T> for PageBuffer<T> {}

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
        pub fn with(value: i32) -> Self {
            Self { data: [value; 4] }
        }

        pub fn one() -> Self {
            Self { data: [1; 4] }
        }

        pub fn two() -> Self {
            Self { data: [2; 4] }
        }
    }

    fn new_5_page_buffer(test_fixture_device: &TestFixtureDevice) -> PageBuffer<Page> {
        PageBuffer::<Page>::new(
            &test_fixture_device.device,
            5,
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!("my_host_visible_buffer"),
        )
        .unwrap()
    }

    #[test]
    fn empty() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let buffer = new_5_page_buffer(&test_fixture_device);

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
        let mut buffer = new_5_page_buffer(&test_fixture_device);

        // Create CommandBuffer
        let mut command_buffer_builder =
            CommandBufferBuilder::begin(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();

        let indices = buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();

        // Submit
        test_fixture_command_buffer
            .queue
            .submit_and_wait_idle(test_fixture_command_buffer.command_buffer)
            .unwrap();

        // Assertions
        assert_eq!(indices.len(), 2);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 1);

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.free_pages(), 3);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn insert_too_many() {
        // Fixtures
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();
        let mut buffer = new_5_page_buffer(&test_fixture_device);

        // Create CommandBuffer
        let mut command_buffer_builder =
            CommandBufferBuilder::begin(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();

        buffer
            .insert(vec![Page::with(0), Page::with(1), Page::with(2)], &mut command_buffer_builder)
            .unwrap();
        let err = buffer
            .insert(vec![Page::with(3), Page::with(4), Page::with(5)], &mut command_buffer_builder)
            .unwrap_err();

        // Assertions
        assert!(matches!(err, Error::WouldOverflow));
    }

    #[test]
    fn free() {
        // Fixtures
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();
        let mut buffer = new_5_page_buffer(&test_fixture_device);

        // Create CommandBuffer
        let mut command_buffer_builder =
            CommandBufferBuilder::begin(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();

        let indices = buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();

        // Free pages
        let count = buffer.free(&indices).unwrap();

        // Assertions
        assert_eq!(count, 2);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.free_pages(), 5);
        assert!(buffer.is_empty());
    }

    #[test]
    fn read_memory() {
        // Fixtures
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();
        let mut buffer = new_5_page_buffer(&test_fixture_device);

        // Command Buffer 1
        let mut command_buffer_builder =
            CommandBufferBuilder::begin(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();

        buffer.insert(vec![Page::one(), Page::two()], &mut command_buffer_builder).unwrap();

        // Submit
        test_fixture_command_buffer
            .queue
            .submit_and_wait_idle(test_fixture_command_buffer.command_buffer)
            .unwrap();

        // Command Buffer 2
        let mut command_buffer = CommandBuffer::new(
            &test_fixture_device.device,
            &test_fixture_command_buffer.command_pool,
            debug_info!("my_command_buffer"),
        )
        .unwrap();
        let mut command_buffer_builder = CommandBufferBuilder::begin(&test_fixture_device.device, &mut command_buffer).unwrap();

        // Read memory back
        let receiver = buffer.read_all(&mut command_buffer_builder).unwrap();

        // Submit
        test_fixture_command_buffer.queue.submit(command_buffer).unwrap();

        // Wait for GPU
        test_fixture_device.device.wait_for_idle().unwrap();
        test_fixture_command_buffer.queue.poll_completed_fences().unwrap();

        // Check result
        let pages = receiver.recv().unwrap();
        assert_eq!(pages.len(), 5);
        assert_eq!(pages[0], Page::one());
        assert_eq!(pages[1], Page::two());
        assert_eq!(pages[2], Page::default());
        assert_eq!(pages[3], Page::default());
        assert_eq!(pages[4], Page::default());
    }
}
