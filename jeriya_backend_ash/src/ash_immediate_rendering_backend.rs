use std::sync::Arc;

use jeriya_shared::{
    immediate::{CommandBufferConfig, Line},
    DebugInfo, ImmediateRenderingBackend,
};

use crate::AshBackend;

pub struct AshImmediateRenderingBackend;

impl AshImmediateRenderingBackend {
    pub fn new() -> Self {
        Self
    }
}

pub struct AshImmediateCommandBuffer {}

impl ImmediateRenderingBackend for AshImmediateRenderingBackend {
    type Backend = AshBackend;

    type CommandBuffer = AshImmediateCommandBuffer;

    fn handle_new(
        &self,
        _backend: &Self::Backend,
        _config: CommandBufferConfig,
        _debug_info: DebugInfo,
    ) -> jeriya_shared::Result<Arc<Self::CommandBuffer>> {
        todo!()
    }

    fn handle_set_config(
        &self,
        _backend: &Self::Backend,
        _command_buffer: &Arc<Self::CommandBuffer>,
        _config: CommandBufferConfig,
    ) -> jeriya_shared::Result<()> {
        todo!()
    }

    fn handle_push_line(
        &self,
        _backend: &Self::Backend,
        _command_buffer: &Arc<Self::CommandBuffer>,
        _line: Line,
    ) -> jeriya_shared::Result<()> {
        todo!()
    }

    fn handle_build(&self, _backend: &Self::Backend, _command_buffer: &Arc<Self::CommandBuffer>) -> jeriya_shared::Result<()> {
        todo!()
    }
}
