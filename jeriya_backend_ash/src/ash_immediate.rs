use std::time::Instant;

use jeriya_backend::immediate::{CommandBuffer, ImmediateRenderingFrame};

/// Stored per update loop to keep track of all the `ImmediateCommandBuffer`s
/// that have to be rendered for that update loop.
pub struct ImmediateRenderingFrameTask {
    /// Time at which the first immediate command buffer was received with the given `ImmediateRenderingFrame`
    pub start_time: Instant,
    /// When the `ImmediateRenderingFrameTask` times out, it is not removed immediately but one frame later.
    pub is_timed_out: bool,
    pub immediate_rendering_frame: ImmediateRenderingFrame,
    pub command_buffers: Vec<CommandBuffer>,
}
