use std::sync::Arc;

use nalgebra::{Vector3, Vector4};

use crate::{
    backend::{Backend, ImmediateRenderingBackend},
    DebugInfo,
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
    command_buffer: Arc<<B as ImmediateRenderingBackend>::CommandBuffer>,
}

impl<'back, B: Backend> CommandBufferBuilder<'back, B> {
    /// Creates a new `CommandBufferBuilder`.
    pub fn new(backend: &'back B, debug_info: DebugInfo) -> crate::Result<Self> {
        let config = Default::default();
        let command_buffer = backend.handle_new(config, debug_info.clone())?;
        Ok(Self { backend, command_buffer })
    }

    /// Sets the config for the `CommandBufferBuilder`.
    pub fn set_config(self, config: CommandBufferConfig) -> crate::Result<Self> {
        self.backend.handle_set_config(&self.command_buffer, config)?;
        Ok(self)
    }

    /// Pushes a new [`Line`] to the `CommandBufferBuilder`.
    pub fn push_line(self, line: impl Into<Line>) -> crate::Result<Self> {
        self.backend.handle_push_line(&self.command_buffer, line.into())?;
        Ok(self)
    }

    /// Finalizes the creation of the command buffer.
    pub fn build(self) -> crate::Result<()> {
        self.backend.handle_build(&self.command_buffer)
    }
}
