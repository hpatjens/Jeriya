use nalgebra::{Vector3, Vector4};

use crate::{
    backend::{Backend, ImmediateRenderingBackend},
    DebugInfo, SubBackendParams,
};

/// Line for immediate rendering
pub struct Line {
    pub v0: Vector3<f32>,
    pub v1: Vector3<f32>,
}

impl Line {
    /// Creates a new `Line` with the given end points
    pub fn new(v0: Vector3<f32>, v1: Vector3<f32>) -> Self {
        Self { v0, v1 }
    }
}

/// Configuration for the [`CommandBufferBuilder`]
pub struct CommandBufferConfig {
    pub default_color: Vector4<f32>,
    pub default_line_width: f32,
}

impl Default for CommandBufferConfig {
    fn default() -> Self {
        Self {
            default_color: Vector4::new(0.5, 0.5, 0.5, 1.0),
            default_line_width: 1.0,
        }
    }
}

/// Creates new command buffers in the [`ImmediateRenderingBackend`].
pub struct CommandBufferBuilder<'back, B: Backend> {
    backend: &'back B,
    config: CommandBufferConfig,
}

impl<'back, B: Backend> CommandBufferBuilder<'back, B> {
    /// Creates a new `CommandBufferBuilder`.
    pub fn new(backend: &'back B, debug_info: DebugInfo) -> crate::Result<Self> {
        let config = Default::default();
        let params = SubBackendParams::new(backend);
        backend
            .immediate_rendering_backend()
            .handle_new(&params, &config, debug_info.clone())?;
        Ok(Self { config, backend })
    }

    /// Sets the config for the `CommandBufferBuilder`.
    pub fn set_config(mut self, config: CommandBufferConfig) -> crate::Result<Self> {
        self.config = config;
        let params = SubBackendParams::new(self.backend);
        self.backend
            .immediate_rendering_backend()
            .handle_set_config(&params, &self.config)?;
        Ok(self)
    }

    /// Pushes a new [`Line`] to the `CommandBufferBuilder`.
    pub fn push_line(self, line: impl Into<Line>) -> crate::Result<Self> {
        let params = SubBackendParams::new(self.backend);
        self.backend
            .immediate_rendering_backend()
            .handle_push_line(&params, &self.config, line.into())?;
        Ok(self)
    }

    /// Finalizes the creation of the command buffer.
    pub fn build(self) -> crate::Result<()> {
        let params = SubBackendParams::new(self.backend);
        self.backend.immediate_rendering_backend().handle_build(&params, &self.config)
    }
}
