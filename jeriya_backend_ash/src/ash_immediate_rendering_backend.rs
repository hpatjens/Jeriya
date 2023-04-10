use jeriya_shared::{
    immediate::{CommandBufferConfig, Line},
    DebugInfo, ImmediateRenderingBackend, SubBackendParams,
};

use crate::AshBackend;

pub struct AshImmediateRenderingBackend;

impl AshImmediateRenderingBackend {
    pub fn new() -> Self {
        Self
    }
}

impl ImmediateRenderingBackend for AshImmediateRenderingBackend {
    type Backend = AshBackend;

    fn handle_new(
        &self,
        params: &SubBackendParams<Self::Backend>,
        config: &CommandBufferConfig,
        debug_info: DebugInfo,
    ) -> jeriya_shared::Result<()> {
        todo!()
    }

    fn handle_set_config(&self, params: &SubBackendParams<Self::Backend>, config: &CommandBufferConfig) -> jeriya_shared::Result<()> {
        todo!()
    }

    fn handle_push_line(
        &self,
        params: &SubBackendParams<Self::Backend>,
        config: &CommandBufferConfig,
        line: Line,
    ) -> jeriya_shared::Result<()> {
        todo!()
    }

    fn handle_build(&self, params: &SubBackendParams<Self::Backend>, config: &CommandBufferConfig) -> jeriya_shared::Result<()> {
        todo!()
    }
}
