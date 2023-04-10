use std::{rc::Rc, sync::Arc};

use ash::vk;

use crate::{device::Device, queue::Queue, AsRawVulkan};

pub enum CommandPoolCreateFlags {
    Transient,
    ResetCommandBuffer,
    Protected,
}

pub struct CommandPool {
    command_pool_create_flags: CommandPoolCreateFlags,
    command_pool: vk::CommandPool,
    device: Arc<Device>,
}

impl CommandPool {
    /// Creates a new `CommandPool` for the given `queue_family_index`
    pub unsafe fn new_from_family(
        device: &Arc<Device>,
        queue_family_index: u32,
        command_pool_create_flags: CommandPoolCreateFlags,
    ) -> crate::Result<Rc<Self>> {
        let vk_command_pool_create_flags = match command_pool_create_flags {
            CommandPoolCreateFlags::Transient => vk::CommandPoolCreateFlags::TRANSIENT,
            CommandPoolCreateFlags::ResetCommandBuffer => vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            CommandPoolCreateFlags::Protected => vk::CommandPoolCreateFlags::PROTECTED,
        };
        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk_command_pool_create_flags)
            .queue_family_index(queue_family_index);
        let command_pool = device.as_raw_vulkan().create_command_pool(&command_pool_create_info, None)?;
        Ok(Rc::new(Self {
            command_pool_create_flags,
            device: device.clone(),
            command_pool,
        }))
    }

    /// Create a new `CommandPool` for the given `queue`.
    pub fn new(device: &Arc<Device>, queue: &Queue, command_pool_create_flags: CommandPoolCreateFlags) -> crate::Result<Rc<Self>> {
        unsafe { Self::new_from_family(device, queue.queue_family_index, command_pool_create_flags) }
    }

    /// Returns the [`CommandPoolCreateFlags`] that were used to create the `CommandPool`.
    pub fn command_pool_create_flags(&self) -> &CommandPoolCreateFlags {
        &self.command_pool_create_flags
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
        use jeriya_test::create_window;

        use crate::{
            command_pool::{CommandPool, CommandPoolCreateFlags},
            device::Device,
            entry::Entry,
            instance::Instance,
            physical_device::PhysicalDevice,
            queue::{Queue, QueueType},
            surface::Surface,
        };

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance, &[surface]).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let presentation_queue = Queue::new(&device, QueueType::Presentation).unwrap();
            let _command_pool = CommandPool::new(&device, &presentation_queue, CommandPoolCreateFlags::ResetCommandBuffer).unwrap();
        }
    }
}
