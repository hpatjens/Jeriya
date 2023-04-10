use std::{rc::Rc, sync::Arc};

use ash::vk::{self};

use crate::{command_pool::CommandPool, device::Device, fence::Fence, AsRawVulkan};

pub struct CommandBuffer {
    completed_fence: Fence,
    command_buffer: vk::CommandBuffer,
    command_pool: Rc<CommandPool>,
    device: Arc<Device>,
}

impl CommandBuffer {
    pub fn new(device: &Arc<Device>, command_pool: &Rc<CommandPool>) -> crate::Result<Self> {
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .command_pool(*command_pool.as_raw_vulkan())
            .level(vk::CommandBufferLevel::PRIMARY);
        let command_buffer = unsafe { device.as_raw_vulkan().allocate_command_buffers(&command_buffer_allocate_info)?[0] };
        let completed_fence = Fence::new(device)?;
        Ok(Self {
            completed_fence,
            command_buffer,
            command_pool: command_pool.clone(),
            device: device.clone(),
        })
    }

    /// The [`CommandPool`] from which the `CommandBuffer` is allocating the commands.
    pub fn command_pool(&self) -> &Rc<CommandPool> {
        &self.command_pool
    }

    /// Fence that signals that the command buffer has completed processing
    pub fn completed_fence(&self) -> &Fence {
        &self.completed_fence
    }
}

impl Drop for CommandBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .as_raw_vulkan()
                .free_command_buffers(*self.command_pool.as_raw_vulkan(), &[self.command_buffer]);
        }
    }
}

impl AsRawVulkan for CommandBuffer {
    type Output = vk::CommandBuffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.command_buffer
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use crate::{
            command_buffer::CommandBuffer,
            command_pool::{CommandPool, CommandPoolCreateFlags},
            device::tests::TestFixtureDevice,
            queue::{Queue, QueueType},
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let presentation_queue = Queue::new(&test_fixture_device.device, QueueType::Presentation).unwrap();
            let command_pool = CommandPool::new(
                &test_fixture_device.device,
                &presentation_queue,
                CommandPoolCreateFlags::ResetCommandBuffer,
            )
            .unwrap();
            let _command_buffer = CommandBuffer::new(&test_fixture_device.device, &command_pool).unwrap();
        }
    }
}
