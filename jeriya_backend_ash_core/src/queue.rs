use std::sync::Arc;

use ash::vk;

use crate::{device::Device, AsRawVulkan};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum QueueType {
    Presentation,
}

pub struct Queue {
    pub queue_family_index: u32,
    pub queue_index: u32,
    queue: vk::Queue,
    device: Arc<Device>,
}

impl AsRawVulkan for Queue {
    type Output = vk::Queue;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.queue
    }
}

impl Queue {
    /// Creates a new `Queue`.
    ///
    /// Safety:
    ///
    /// The `queue_family_index` and `queue_index` must be correct.
    pub(crate) unsafe fn get_from_family(device: &Arc<Device>, queue_family_index: u32, queue_index: u32) -> Self {
        let vk_queue = device.as_raw_vulkan().get_device_queue(queue_family_index, queue_index);
        Self {
            queue_family_index,
            queue_index,
            queue: vk_queue,
            device: device.clone(),
        }
    }

    /// Creates a new `Queue` with the given [`QueueType`]
    pub fn new(device: &Arc<Device>, queue_type: QueueType) -> crate::Result<Self> {
        match queue_type {
            QueueType::Presentation => {
                assert!(!device.physical_device.suitable_presentation_graphics_queue_family_infos.is_empty());
                assert!(device.physical_device.suitable_presentation_graphics_queue_family_infos[0].queue_count > 0);
                let queue_family_index = device.physical_device.suitable_presentation_graphics_queue_family_infos[0].queue_family_index;
                let queue_index = 0;
                unsafe { Ok(Queue::get_from_family(&device, queue_family_index, queue_index)) }
            }
        }
    }
}
