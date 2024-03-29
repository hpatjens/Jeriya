use std::{mem, sync::Arc};

use crate::{
    buffer::{Buffer, BufferUsageFlags, GeneralBuffer},
    command_buffer_builder::CommandBufferBuilder,
    device::Device,
    device_visible_buffer::DeviceVisibleBuffer,
    host_visible_buffer::HostVisibleBuffer,
    AsRawVulkan, Error,
};
use ash::vk;
use jeriya_shared::{debug_info, AsDebugInfo, DebugInfo};

/// Device visible buffer of a constant size which can be filled by pushing chunks of data to it via a staging buffer.
pub struct StagedPushOnlyBuffer<T> {
    device_visible_buffer: Arc<DeviceVisibleBuffer<T>>,
    capacity: usize,
    len: usize,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl<T: Clone + 'static + Send + Sync> StagedPushOnlyBuffer<T> {
    /// Creates a new [`StagedPushOnlyBuffer`] with the given `size` and `device_buffer_usage_flags`. Size is not measured in bytes but in the number of elements of type `T`.
    pub fn new(
        device: &Arc<Device>,
        size: usize,
        device_buffer_usage_flags: BufferUsageFlags,
        debug_info: DebugInfo,
    ) -> crate::Result<Self> {
        let device_visible_buffer = DeviceVisibleBuffer::new(
            device,
            size * mem::size_of::<T>(),
            device_buffer_usage_flags | BufferUsageFlags::TRANSFER_DST_BIT | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info.clone(),
        )?;
        Ok(Self {
            device_visible_buffer,
            device: device.clone(),
            capacity: size,
            len: 0,
            debug_info,
        })
    }

    /// Copies the `data` into a newly constructed [`HostVisibleBuffer`] and issues a copy command to the [`CommandBufferBuilder`] to copy the data from the [`HostVisibleBuffer`] to the [`DeviceVisibleBuffer`].
    pub fn push(&mut self, data: &[T], command_buffer_builder: &mut CommandBufferBuilder) -> crate::Result<Option<usize>> {
        if data.is_empty() {
            return Ok(None);
        }
        if self.len + data.len() > self.capacity {
            return Err(Error::WouldOverflow);
        }
        let host_visible_buffer = Arc::new(HostVisibleBuffer::<T>::new(
            &self.device,
            data,
            BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!("PushOnlyBuffer"),
        )?);

        // Copy the data from the host visible buffer to the device visible buffer
        let data_byte_size = mem::size_of_val(data);
        let data_offset = self.len * mem::size_of::<T>();
        let command_buffer = command_buffer_builder.command_buffer();
        unsafe {
            let copy_region = vk::BufferCopy {
                src_offset: 0,
                dst_offset: data_offset as u64,
                size: data_byte_size as u64,
            };
            self.device.as_raw_vulkan().cmd_copy_buffer(
                *command_buffer.as_raw_vulkan(),
                *host_visible_buffer.as_raw_vulkan(),
                *self.device_visible_buffer.as_raw_vulkan(),
                &[copy_region],
            );
            command_buffer.push_dependency(host_visible_buffer.clone());
            command_buffer.push_dependency(self.device_visible_buffer.clone());
        }
        let offset = self.len;
        self.len += data.len();
        Ok(Some(offset))
    }

    /// Returns the length of the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the buffer.
    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T: Clone + 'static + Default + Send + Sync> StagedPushOnlyBuffer<T> {
    /// Reads all data from the [`DeviceVisibleBuffer`] into a newly constructed [`HostVisibleBuffer`] and issues a copy command to the [`CommandBufferBuilder`] to copy the data from the [`DeviceVisibleBuffer`] to the [`HostVisibleBuffer`].
    #[cfg(test)]
    pub fn read_all(&mut self, command_buffer_builder: &mut CommandBufferBuilder) -> crate::Result<std::sync::mpsc::Receiver<Vec<T>>> {
        use jeriya_shared::parking_lot::Mutex;
        let host_visible_buffer = Arc::new(Mutex::new(HostVisibleBuffer::<T>::new(
            &self.device,
            &vec![Default::default(); self.len],
            BufferUsageFlags::TRANSFER_DST_BIT,
            debug_info!("PushOnlyBuffer"),
        )?));
        let byte_size = self.len * mem::size_of::<T>();
        command_buffer_builder.copy_buffer_range_from_device_to_host(&self.device_visible_buffer, 0, &host_visible_buffer, 0, byte_size);

        // Enqueue finished operation to get the data from the host visible buffer.
        let len = self.len;
        let (sender, receiver) = std::sync::mpsc::channel();
        command_buffer_builder.push_finished_operation(Box::new(move || {
            let mut data = vec![Default::default(); len];
            let host_visible_buffer = host_visible_buffer.lock();
            host_visible_buffer.get_memory_unaligned(&mut data)?;
            sender
                .send(data)
                .expect("Failed to send data from StagedPushOnlyBuffer to receiver in finished operation.");
            Ok(())
        }));
        Ok(receiver)
    }
}

impl<T> AsDebugInfo for StagedPushOnlyBuffer<T> {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl<T> AsRawVulkan for StagedPushOnlyBuffer<T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        self.device_visible_buffer.as_raw_vulkan()
    }
}

impl<T> GeneralBuffer for StagedPushOnlyBuffer<T> {}
impl<T> Buffer<T> for StagedPushOnlyBuffer<T> {}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_shared::debug_info;

        use crate::{
            buffer::BufferUsageFlags,
            command_buffer::{tests::TestFixtureCommandBuffer, CommandBuffer},
            command_buffer_builder::CommandBufferBuilder,
            device::TestFixtureDevice,
            staged_push_only_buffer::StagedPushOnlyBuffer,
            Error,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let mut test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();

            let mut buffer = StagedPushOnlyBuffer::<f32>::new(
                &test_fixture_device.device,
                4,
                BufferUsageFlags::STORAGE_BUFFER,
                debug_info!("my_host_visible_buffer"),
            )
            .unwrap();
            assert!(buffer.is_empty());
            assert_eq!(buffer.len(), 0);
            assert_eq!(buffer.capacity(), 4);

            let mut command_buffer_builder =
                CommandBufferBuilder::new(&test_fixture_device.device, &mut test_fixture_command_buffer.command_buffer).unwrap();
            command_buffer_builder.begin_command_buffer().unwrap();

            let offset1 = buffer.push(&[0.0, 0.0], &mut command_buffer_builder).unwrap().unwrap();
            assert_eq!(offset1, 0);
            assert_eq!(buffer.len(), 2);

            let offset2 = buffer.push(&[1.0, 1.0], &mut command_buffer_builder).unwrap().unwrap();
            assert_eq!(offset2, 2);
            assert_eq!(buffer.len(), 4);

            let result = buffer.push(&[2.0], &mut command_buffer_builder);
            assert!(matches!(result, Err(Error::WouldOverflow)));

            command_buffer_builder.end_command_buffer().unwrap();

            // Wait for GPU
            test_fixture_command_buffer
                .queue
                .submit(test_fixture_command_buffer.command_buffer)
                .unwrap();

            // Command Buffer 2
            let mut command_buffer = CommandBuffer::new(
                &test_fixture_device.device,
                &test_fixture_command_buffer.command_pool,
                debug_info!("my_command_buffer"),
            )
            .unwrap();
            let mut command_buffer_builder = CommandBufferBuilder::new(&test_fixture_device.device, &mut command_buffer).unwrap();
            command_buffer_builder.begin_command_buffer().unwrap();
            let receiver = buffer.read_all(&mut command_buffer_builder).unwrap();
            command_buffer_builder.end_command_buffer().unwrap();

            // Wait for GPU
            test_fixture_command_buffer.queue.submit(command_buffer).unwrap();
            test_fixture_device.device.wait_for_idle().unwrap();
            test_fixture_command_buffer.queue.poll_completed_fences().unwrap();

            let read_data = receiver.recv().unwrap();
            assert_eq!(read_data, vec![0.0, 0.0, 1.0, 1.0]);
        }
    }
}
