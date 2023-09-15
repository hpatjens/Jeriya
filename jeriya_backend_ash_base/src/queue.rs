use std::{collections::VecDeque, marker::PhantomData, sync::Arc};

use ash::vk;
use jeriya_shared::{
    log::{info, trace},
    thread_id,
    tracy_client::span,
    AsDebugInfo, DebugInfo,
};

use crate::{command_buffer::CommandBuffer, device::Device, fence::Fence, semaphore::Semaphore, AsRawVulkan, DebugInfoAshExtension};

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
    debug_info: DebugInfo,
    phantom_data: PhantomData<*const ()>, // Making this !Send
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
            phantom_data: PhantomData,
        }
    }

    /// Creates a new `Queue` with the given [`QueueType`]
    pub fn new(device: &Arc<Device>, queue_type: QueueType, queue_index: u32, debug_info: DebugInfo) -> crate::Result<Self> {
        match queue_type {
            QueueType::Presentation => {
                assert!(!device.physical_device.suitable_presentation_graphics_queue_family_infos.is_empty());
                assert!(device.physical_device.suitable_presentation_graphics_queue_family_infos[0].queue_count > queue_index);
                let queue_family_index = device.physical_device.suitable_presentation_graphics_queue_family_infos[0].queue_family_index;
                unsafe { Ok(Queue::get_from_family(device, queue_family_index, queue_index, debug_info)) }
            }
        }
    }

    /// Submits the given [`CommandBuffer`] to the `Queue`.
    pub fn submit(&mut self, command_buffer: CommandBuffer) -> crate::Result<()> {
        trace!(
            "Queue ({:?}) submit on thread: {:?} with id {}",
            self.debug_info.ptr,
            std::thread::current().name().unwrap_or("unnamed thread"),
            thread_id::get(),
        );

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
        trace!(
            "Queue ({:?}) submit on thread: {:?} with id {}",
            self.debug_info.ptr,
            std::thread::current().name().unwrap_or("unnamed thread"),
            thread_id::get(),
        );

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

        use crate::device::tests::TestFixtureDevice;

        use super::*;

        #[test]
        fn smoke() {
            let device_test_fixture = TestFixtureDevice::new().unwrap();
            let _queue = Queue::new(&device_test_fixture.device, QueueType::Presentation, 0, debug_info!("my_queue")).unwrap();
        }
    }
}
