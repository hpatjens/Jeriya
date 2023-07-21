use std::{collections::VecDeque, sync::Arc};

use ash::vk;
use jeriya_shared::tracy_client::span;

use crate::{command_buffer::CommandBuffer, device::Device, fence::Fence, semaphore::Semaphore, AsRawVulkan};

pub enum SubmittedCommandBuffer {
    Value(CommandBuffer),
    Arc {
        semaphores: Vec<Arc<Semaphore>>,
        command_buffer: Arc<CommandBuffer>,
    },
}

impl SubmittedCommandBuffer {
    fn completed_fence(&self) -> &Fence {
        match self {
            SubmittedCommandBuffer::Value(command_buffer) => command_buffer.completed_fence(),
            SubmittedCommandBuffer::Arc { command_buffer, .. } => command_buffer.completed_fence(),
        }
    }

    fn command_buffer(&self) -> &CommandBuffer {
        match self {
            SubmittedCommandBuffer::Value(command_buffer) => command_buffer,
            SubmittedCommandBuffer::Arc { command_buffer, .. } => command_buffer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum QueueType {
    Presentation,
}
pub struct Queue {
    pub queue_family_index: u32,
    pub queue_index: u32,
    pub submitted_command_buffers: VecDeque<SubmittedCommandBuffer>,
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
            submitted_command_buffers: VecDeque::new(),
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
                unsafe { Ok(Queue::get_from_family(device, queue_family_index, queue_index)) }
            }
        }
    }

    /// Submits the given [`CommandBuffer`] to the `Queue`.
    pub fn submit(&mut self, command_buffer: CommandBuffer) -> crate::Result<()> {
        let command_buffers = [*command_buffer.as_raw_vulkan()];
        let submit_infos = [vk::SubmitInfo::builder().command_buffers(&command_buffers).build()];
        unsafe {
            self.device
                .as_raw_vulkan()
                .queue_submit(self.queue, &submit_infos, *command_buffer.completed_fence().as_raw_vulkan())?;
        }
        self.submitted_command_buffers
            .push_back(SubmittedCommandBuffer::Value(command_buffer));
        Ok(())
    }

    /// Submits the given [`CommandBuffer`] to the `Queue` and waits for the given [`Semaphore`] to be signalled.
    pub fn submit_for_rendering_complete(
        &mut self,
        command_buffer: Arc<CommandBuffer>,
        wait_semaphore: &Arc<Semaphore>,
        signal_semaphore: &Arc<Semaphore>,
    ) -> crate::Result<()> {
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&[*wait_semaphore.as_raw_vulkan()])
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .command_buffers(&[*command_buffer.as_raw_vulkan()])
            .signal_semaphores(&[*signal_semaphore.as_raw_vulkan()])
            .build();
        unsafe {
            self.device
                .as_raw_vulkan()
                .queue_submit(self.queue, &[submit_info], *command_buffer.completed_fence().as_raw_vulkan())?
        };
        self.submitted_command_buffers.push_back(SubmittedCommandBuffer::Arc {
            command_buffer,
            semaphores: vec![signal_semaphore.clone(), wait_semaphore.clone()],
        });
        Ok(())
    }

    /// Polls the fences that signal the completion of the submitted [`CommandBuffer`]s and executes the finished operations of the [`CommandBuffer`]s that have finished executing.s
    pub fn poll_completed_fences(&mut self) -> crate::Result<()> {
        let _span = span!("poll_completed_fences");
        loop {
            let result = self
                .submitted_command_buffers
                .front()
                .map(|first| first.completed_fence().get_fence_status())
                .unwrap_or(Ok(false))?;
            if result {
                let finished_command_buffer = self.submitted_command_buffers.pop_front();
                if let Some(finished_command_buffer) = &finished_command_buffer {
                    for finished_operation in finished_command_buffer.command_buffer().finished_operations() {
                        finished_operation()?;
                    }
                }
                drop(finished_command_buffer);
            } else {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod new {
        use crate::device::tests::TestFixtureDevice;

        use super::*;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let _queue = Queue::new(&device_test_fixture.device, QueueType::Presentation).unwrap();
        }
    }
}
