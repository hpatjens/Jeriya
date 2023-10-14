use std::sync::Arc;

use ash::vk;
use jeriya_shared::{AsDebugInfo, DebugInfo};

use crate::{device::Device, queue::Queue, AsRawVulkan, DebugInfoAshExtension};

pub enum CommandPoolCreateFlags {
    Transient,
    ResetCommandBuffer,
    Protected,
}

pub struct CommandPool {
    command_pool_create_flags: CommandPoolCreateFlags,
    command_pool: vk::CommandPool,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl CommandPool {
    /// Creates a new `CommandPool` for the given `queue_family_index`
    pub unsafe fn new_from_family(
        device: &Arc<Device>,
        queue_family_index: u32,
        command_pool_create_flags: CommandPoolCreateFlags,
        debug_info: DebugInfo,
    ) -> crate::Result<Arc<Self>> {
        let vk_command_pool_create_flags = match command_pool_create_flags {
            CommandPoolCreateFlags::Transient => vk::CommandPoolCreateFlags::TRANSIENT,
            CommandPoolCreateFlags::ResetCommandBuffer => vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            CommandPoolCreateFlags::Protected => vk::CommandPoolCreateFlags::PROTECTED,
        };
        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk_command_pool_create_flags)
            .queue_family_index(queue_family_index);
        let command_pool = device.as_raw_vulkan().create_command_pool(&command_pool_create_info, None)?;
        let debug_info = debug_info.with_vulkan_ptr(command_pool);
        Ok(Arc::new(Self {
            command_pool_create_flags,
            device: device.clone(),
            command_pool,
            debug_info,
        }))
    }

    /// Create a new `CommandPool` for the given `queue`.
    pub fn new(
        device: &Arc<Device>,
        queue: &Queue,
        command_pool_create_flags: CommandPoolCreateFlags,
        debug_info: DebugInfo,
    ) -> crate::Result<Arc<Self>> {
        unsafe { Self::new_from_family(device, queue.queue_family_index, command_pool_create_flags, debug_info) }
    }

    /// Returns the [`CommandPoolCreateFlags`] that were used to create the `CommandPool`.
    pub fn command_pool_create_flags(&self) -> &CommandPoolCreateFlags {
        &self.command_pool_create_flags
    }
}

impl AsDebugInfo for CommandPool {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl AsRawVulkan for CommandPool {
    type Output = vk::CommandPool;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.command_pool
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe {
            self.device.as_raw_vulkan().destroy_command_pool(self.command_pool, None);
        }
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_shared::debug_info;

        use crate::{
            command_pool::{CommandPool, CommandPoolCreateFlags},
            device::TestFixtureDevice,
            queue::Queue,
            queue_plan::QueueSelection,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let presentation_queue = Queue::new(
                &test_fixture_device.device,
                &QueueSelection::new_unchecked(0, 0),
                debug_info!("my_queue"),
            )
            .unwrap();
            let _command_pool = CommandPool::new(
                &test_fixture_device.device,
                &presentation_queue,
                CommandPoolCreateFlags::ResetCommandBuffer,
                debug_info!("my_command_pool"),
            )
            .unwrap();
        }
    }
}
