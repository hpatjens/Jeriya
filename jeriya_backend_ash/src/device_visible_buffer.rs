use std::sync::Arc;

use ash::vk;
use jeriya_shared::{AsDebugInfo, DebugInfo};

use crate::{
    buffer::{Buffer, BufferUsageFlags, GeneralBuffer},
    device::Device,
    unsafe_buffer::UnsafeBuffer,
    AsRawVulkan,
};

#[cfg(test)]
use crate::{
    command_buffer::CommandBuffer, command_buffer_builder::CommandBufferBuilder, command_pool::CommandPool,
    host_visible_buffer::HostVisibleBuffer, queue::Queue,
};

pub struct DeviceVisibleBuffer<T> {
    buffer: UnsafeBuffer<T>,
    _device: Arc<Device>,
}

impl<T: Clone + 'static + Send + Sync> DeviceVisibleBuffer<T> {
    /// Creates a new `DeviceVisibleBuffer`.
    pub fn new(
        device: &Arc<Device>,
        byte_size: usize,
        buffer_usage_flags: BufferUsageFlags,
        debug_info: DebugInfo,
    ) -> crate::Result<Arc<Self>> {
        let buffer = unsafe {
            let mut buffer = UnsafeBuffer::new(
                device,
                byte_size,
                buffer_usage_flags.into(),
                vk::SharingMode::CONCURRENT,
                debug_info,
            )?;
            buffer.allocate_memory(vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
            buffer
        };
        Ok(Arc::new(Self {
            _device: device.clone(),
            buffer,
        }))
    }

    /// Creates a new DeviceVisibleBuffer and transfers the data from the given [`HostVisibleBuffer`] to it by submitting a [`CommandBuffer`] to the given transfer queue.
    #[cfg(test)]
    pub fn new_and_transfer_from_host_visible(
        device: &Arc<Device>,
        source_buffer: &Arc<HostVisibleBuffer<T>>,
        transfer_queue: &mut Queue,
        command_pool: &Arc<CommandPool>,
        buffer_usage_flags: BufferUsageFlags,
        debug_info: DebugInfo,
    ) -> crate::Result<Arc<Self>> {
        let result = Self::new(device, source_buffer.byte_size(), buffer_usage_flags, debug_info)?;
        result.transfer_memory_with_command_buffer(source_buffer, transfer_queue, command_pool)?;
        Ok(result)
    }

    /// Transfers the data from the [`HostVisibleBuffer`] to the [`DeviceVisibleBuffer`] by submitting a [`CommandBuffer`] to the given transfer [`Queue`].
    #[cfg(test)]
    pub fn transfer_memory_with_command_buffer(
        self: &Arc<Self>,
        source_buffer: &Arc<HostVisibleBuffer<T>>,
        transfer_queue: &mut Queue,
        command_pool: &Arc<CommandPool>,
    ) -> crate::Result<()> {
        use jeriya_shared::debug_info;

        let mut command_buffer = CommandBuffer::new(&self._device, command_pool, debug_info!("CommandBuffer-for-DeviceVisibleBuffer"))?;
        CommandBufferBuilder::new(&self._device, &mut command_buffer)?
            .begin_command_buffer()?
            .copy_buffer_from_host_to_device(source_buffer, self)
            .end_command_buffer()?;
        transfer_queue.submit(command_buffer)?;
        Ok(())
    }

    /// Size of the buffer in bytes
    pub fn byte_size(&self) -> usize {
        self.buffer.byte_size()
    }
}

impl<T> GeneralBuffer for DeviceVisibleBuffer<T> {}
impl<T> Buffer<T> for DeviceVisibleBuffer<T> {}

impl<T> AsRawVulkan for DeviceVisibleBuffer<T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        self.buffer.as_raw_vulkan()
    }
}

impl<T> AsDebugInfo for DeviceVisibleBuffer<T> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.buffer.as_debug_info()
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::sync::Arc;

        use jeriya_shared::debug_info;

        use crate::{
            buffer::BufferUsageFlags,
            command_pool::{CommandPool, CommandPoolCreateFlags},
            device::TestFixtureDevice,
            device_visible_buffer::DeviceVisibleBuffer,
            host_visible_buffer::HostVisibleBuffer,
            queue::Queue,
            queue_plan::QueueSelection,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let mut presentation_queue = Queue::new(
                &test_fixture_device.device,
                &QueueSelection::new_unchecked(0, 0),
                debug_info!("my_queue"),
            )
            .unwrap();
            let command_pool = CommandPool::new(
                &test_fixture_device.device,
                &presentation_queue,
                CommandPoolCreateFlags::ResetCommandBuffer,
                debug_info!("my_command_pool"),
            )
            .unwrap();
            let host_visible_buffer = Arc::new(
                HostVisibleBuffer::<f32>::new(
                    &test_fixture_device.device,
                    &[1.0, 2.0, 3.0],
                    BufferUsageFlags::VERTEX_BUFFER,
                    debug_info!("my_host_visible_buffer"),
                )
                .unwrap(),
            );
            let _device_visible_buffer = DeviceVisibleBuffer::new_and_transfer_from_host_visible(
                &test_fixture_device.device,
                &host_visible_buffer,
                &mut presentation_queue,
                &command_pool,
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("my_device_visible_buffer"),
            )
            .unwrap();
        }
    }
}
