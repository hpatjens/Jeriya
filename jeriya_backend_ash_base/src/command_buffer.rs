use std::sync::Arc;

use ash::vk;
use jeriya_shared::{debug_info, AsDebugInfo, DebugInfo};

use crate::{command_pool::CommandPool, device::Device, fence::Fence, AsRawVulkan, DebugInfoAshExtension};

pub trait CommandBufferDependency: Send + Sync {}

pub type FinishedOperation = Box<dyn Fn() -> crate::Result<()> + 'static + Send + Sync>;

pub struct CommandBuffer {
    completed_fence: Fence,
    command_buffer: vk::CommandBuffer,
    command_pool: Arc<CommandPool>,
    dependencies: Vec<Arc<dyn CommandBufferDependency>>,
    finished_operations: Vec<FinishedOperation>,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl CommandBuffer {
    pub fn new(device: &Arc<Device>, command_pool: &Arc<CommandPool>, debug_info: DebugInfo) -> crate::Result<Self> {
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .command_pool(*command_pool.as_raw_vulkan())
            .level(vk::CommandBufferLevel::PRIMARY);
        let command_buffer = unsafe { device.as_raw_vulkan().allocate_command_buffers(&command_buffer_allocate_info)?[0] };
        let completed_fence = Fence::new(device, debug_info!("CommandBuffer-completed-Fence"))?;
        let debug_info = debug_info.with_vulkan_ptr(command_buffer);
        Ok(Self {
            completed_fence,
            command_buffer,
            command_pool: command_pool.clone(),
            dependencies: Vec::new(),
            finished_operations: Vec::new(),
            device: device.clone(),
            debug_info,
        })
    }

    /// Returns the finished operations of the `CommandBuffer`.
    pub(crate) fn finished_operations(&self) -> &Vec<FinishedOperation> {
        &self.finished_operations
    }

    /// Pushes a function that will be called in the `CommandBuffer::finish` method when the `CommandBuffer` has been processed.
    pub(crate) fn push_finished_operation(&mut self, finished_operation: FinishedOperation) {
        self.finished_operations.push(finished_operation);
    }

    /// The [`CommandPool`] from which the `CommandBuffer` is allocating the commands.
    pub fn command_pool(&self) -> &Arc<CommandPool> {
        &self.command_pool
    }

    /// Fence that signals that the command buffer has completed processing
    pub fn completed_fence(&self) -> &Fence {
        &self.completed_fence
    }

    /// Wait for the [`CommandBuffer`] to complete processing
    pub fn wait_for_completion(&self) -> crate::Result<()> {
        self.completed_fence.wait()
    }

    /// Adds a dependency to the command buffer. The dependency well be kept alive until the command buffer is dropped.
    pub fn push_dependency(&mut self, dependency: Arc<dyn CommandBufferDependency>) {
        self.dependencies.push(dependency);
    }
}

impl AsDebugInfo for CommandBuffer {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
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
pub mod tests {
    use std::sync::Arc;

    use jeriya_shared::debug_info;

    use crate::{
        command_buffer::CommandBuffer,
        command_pool::{CommandPool, CommandPoolCreateFlags},
        device::tests::TestFixtureDevice,
        queue::{Queue, QueueType},
    };

    pub struct TestFixtureCommandBuffer {
        pub queue: Queue,
        pub command_pool: Arc<CommandPool>,
        pub command_buffer: CommandBuffer,
    }

    impl TestFixtureCommandBuffer {
        pub fn new(test_fixture_device: &TestFixtureDevice) -> crate::Result<Self> {
            let queue = Queue::new(&test_fixture_device.device, QueueType::Presentation, 0, debug_info!("my_queue")).unwrap();
            let command_pool = CommandPool::new(
                &test_fixture_device.device,
                &queue,
                CommandPoolCreateFlags::ResetCommandBuffer,
                debug_info!("my_command_pool"),
            )
            .unwrap();
            let command_buffer = CommandBuffer::new(&test_fixture_device.device, &command_pool, debug_info!("my_command_buffer")).unwrap();
            Ok(Self {
                queue,
                command_pool,
                command_buffer,
            })
        }
    }

    #[test]
    fn smoke() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let _test_fixture_command_buffer = TestFixtureCommandBuffer::new(&test_fixture_device).unwrap();
    }
}
