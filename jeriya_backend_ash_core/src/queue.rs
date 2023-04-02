use ash::vk;

use crate::AsRawVulkan;

pub struct Queue {
    pub queue_family_index: u32,
    pub queue_index: u32,
    queue: vk::Queue,
}

impl AsRawVulkan for Queue {
    type Output = vk::Queue;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.queue
    }
}

impl Queue {
    /// Creates a new queue.
    ///
    /// Safety:
    ///
    /// The `queue_family_index` and `queue_index` must be correct.
    pub(crate) unsafe fn get_from_family(device: &ash::Device, queue_family_index: u32, queue_index: u32) -> Self {
        let vk_queue = device.get_device_queue(queue_family_index, queue_index);
        Self {
            queue_family_index,
            queue_index,
            queue: vk_queue,
        }
    }
}
