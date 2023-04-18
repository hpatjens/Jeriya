use nalgebra::{Vector3, Vector4};

use crate::{backend::Backend, DebugInfo, ImmediateCommandBufferBuilder};

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

/// Creates new command buffer in the [`ImmediateRenderingBackend`].
pub struct CommandBufferBuilder<B: Backend> {
    command_buffer_builder: B::ImmediateCommandBufferBuilder,
}

impl<B: Backend> CommandBufferBuilder<B> {
    /// Creates a new `CommandBufferBuilder`.
    pub fn new(backend: &B, debug_info: DebugInfo) -> crate::Result<Self> {
        let command_buffer_builder = backend.create_immediate_command_buffer_builder(CommandBufferConfig::default(), debug_info)?;
        Ok(Self {
            command_buffer_builder: command_buffer_builder,
        })
    }

    /// Sets the config for the `CommandBufferBuilder`.
    pub fn set_config(mut self, config: CommandBufferConfig) -> crate::Result<Self> {
        self.command_buffer_builder.set_config(config)?;
        Ok(self)
    }

    /// Pushes a new [`Line`] to the `CommandBufferBuilder`.
    pub fn push_line(mut self, line: impl Into<Line>) -> crate::Result<Self> {
        self.command_buffer_builder.push_line(line.into())?;
        Ok(self)
    }

    /// Finalizes the creation of the command buffer.
    pub fn build(self) -> crate::Result<()> {
        self.command_buffer_builder.build()
    }
}
