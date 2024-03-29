use std::{collections::VecDeque, sync::Arc};

use ash::vk;
use jeriya_shared::{log::info, AsDebugInfo, DebugInfo};

use crate::{
    command_buffer::{CommandBuffer, CommandBufferState},
    device::Device,
    fence::Fence,
    queue_plan::QueueSelection,
    semaphore::Semaphore,
    AsRawVulkan, DebugInfoAshExtension,
};

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
pub enum QueueType {}

pub struct Queue {
    pub queue_family_index: u32,
    pub queue_index: u32,
    pub submitted_command_buffers: VecDeque<SubmittedCommandBuffer>,
    queue: vk::Queue,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl AsRawVulkan for Queue {
    type Output = vk::Queue;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.queue
    }
}

impl AsDebugInfo for Queue {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl Queue {
    /// Creates a new `Queue`.
    ///
    /// Safety:
    ///
    /// The `queue_family_index` and `queue_index` must be correct.
    pub(crate) unsafe fn get_from_family(device: &Arc<Device>, queue_family_index: u32, queue_index: u32, debug_info: DebugInfo) -> Self {
        let vk_queue = device.as_raw_vulkan().get_device_queue(queue_family_index, queue_index);
        let debug_info = debug_info.with_vulkan_ptr(vk_queue);
        info! {
            "Creating queue with queue_family_index: {}, queue_index: {}: {}",
            queue_family_index,
            queue_index,
            debug_info.format_one_line()
        }
        Self {
            queue_family_index,
            queue_index,
            submitted_command_buffers: VecDeque::new(),
            queue: vk_queue,
            device: device.clone(),
            debug_info,
        }
    }

    /// Creates a new `Queue`
    pub fn new(device: &Arc<Device>, queue_selection: &QueueSelection, debug_info: DebugInfo) -> crate::Result<Self> {
        unsafe {
            Ok(Queue::get_from_family(
                device,
                queue_selection.queue_family_index(),
                queue_selection.queue_index(),
                debug_info,
            ))
        }
    }

    /// Submits the given [`CommandBuffer`] to the `Queue`.
    pub fn submit(&mut self, mut command_buffer: CommandBuffer) -> crate::Result<()> {
        // In case the command buffer was not ended explicitly
        if command_buffer.state() == CommandBufferState::Recording {
            command_buffer.end()?;
        }

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

    /// Submits the given [`CommandBuffer`] to the `Queue` and waits for the `Queue` to be idle.
    pub fn submit_and_wait_idle(&mut self, command_buffer: CommandBuffer) -> crate::Result<()> {
        self.submit(command_buffer)?;
        self.wait_idle()
    }

    /// Submits the given [`CommandBuffer`] to the `Queue` and waits for the given [`Semaphore`] to be signalled.
    ///
    /// All passed resources must be held alive until the `Queue` has finished using them.
    pub fn submit_for_rendering_complete(
        &mut self,
        command_buffer: &CommandBuffer,
        wait_semaphore: &Semaphore,
        signal_semaphore: &Semaphore,
        fence: &Fence,
    ) -> crate::Result<()> {
        let wait_semaphores = [*wait_semaphore.as_raw_vulkan()];
        let signal_semaphores = [*signal_semaphore.as_raw_vulkan()];
        let command_buffers = [*command_buffer.as_raw_vulkan()];
        let wait_dst_stage_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_dst_stage_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)
            .build();
        unsafe {
            self.device
                .as_raw_vulkan()
                .queue_submit(self.queue, &[submit_info], *fence.as_raw_vulkan())?
        };
        Ok(())
    }

    /// Polls the fences that signal the completion of the submitted [`CommandBuffer`]s and executes the finished operations of the [`CommandBuffer`]s that have finished executing.s
    pub fn poll_completed_fences(&mut self) -> crate::Result<()> {
        let _span = jeriya_shared::span!("poll_completed_fences");
        loop {
            let result = self
                .submitted_command_buffers
                .front()
                .map(|first| first.completed_fence().get_fence_status())
                .unwrap_or(Ok(false))?;
            if result {
                let mut finished_command_buffer = self.submitted_command_buffers.pop_front();
                if let Some(finished_command_buffer) = finished_command_buffer.as_mut() {
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

    /// Waits for the `Queue` to be idle.
    pub fn wait_idle(&self) -> crate::Result<()> {
        unsafe { self.device.as_raw_vulkan().queue_wait_idle(self.queue) }?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod new {
        use jeriya_shared::debug_info;

        use crate::device::TestFixtureDevice;

        use super::*;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let _queue = Queue::new(
                &device_test_fixture.device,
                &device_test_fixture.device.queue_plan.presentation_queues[0],
                debug_info!("my_queue"),
            )
            .unwrap();
        }
    }
}
