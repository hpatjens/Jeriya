use std::{rc::Rc, sync::Arc};

use ash::vk;
use jeriya_shared::{debug_info, AsDebugInfo, DebugInfo};

use crate::{
    buffer::BufferUsageFlags,
    command_buffer::{CommandBuffer, CommandBufferDependency},
    command_buffer_builder::CommandBufferBuilder,
    command_pool::CommandPool,
    device::Device,
    host_visible_buffer::HostVisibleBuffer,
    queue::Queue,
    unsafe_buffer::UnsafeBuffer,
    AsRawVulkan,
};

pub struct DeviceVisibleBuffer<T> {
    buffer: UnsafeBuffer<T>,
}

impl<T: Copy + 'static> DeviceVisibleBuffer<T> {
    pub fn new(
        device: &Arc<Device>,
        source_buffer: &Arc<HostVisibleBuffer<T>>,
        transfer_queue: &mut Queue,
        command_pool: &Rc<CommandPool>,
        buffer_usage_flags: BufferUsageFlags,
        debug_info: DebugInfo,
    ) -> crate::Result<Arc<Self>> {
        let buffer = unsafe {
            let mut buffer = UnsafeBuffer::new(
                device,
                source_buffer.byte_size(),
                buffer_usage_flags.into(),
                vk::SharingMode::EXCLUSIVE,
                debug_info,
            )?;
            buffer.allocate_memory(vk::MemoryPropertyFlags::HOST_VISIBLE)?;
            buffer
        };
        let mut command_buffer = CommandBuffer::new(device, command_pool, debug_info!("CommandBuffer-for-DeviceVisibleBuffer"))?;
        let result = Arc::new(Self { buffer });
        CommandBufferBuilder::new(device, &mut command_buffer)?
            .begin_command_buffer()?
            .copy_buffer_from_host_to_device(source_buffer, &result)
            .end_command_buffer()?;
        transfer_queue.submit(command_buffer)?;
        Ok(result)
    }

    /// Size of the buffer in bytes
    pub fn byte_size(&self) -> usize {
        self.buffer.byte_size()
    }
}

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

impl<T> CommandBufferDependency for DeviceVisibleBuffer<T> {}

#[cfg(test)]
mod tests {
    mod new {
        use std::sync::Arc;

        use jeriya_shared::debug_info;

        use crate::{
            buffer::BufferUsageFlags,
            command_pool::{CommandPool, CommandPoolCreateFlags},
            device::tests::TestFixtureDevice,
            device_visible_buffer::DeviceVisibleBuffer,
            host_visible_buffer::HostVisibleBuffer,
            queue::{Queue, QueueType},
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let mut presentation_queue = Queue::new(&test_fixture_device.device, QueueType::Presentation).unwrap();
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
            let _device_visible_buffer = DeviceVisibleBuffer::new(
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