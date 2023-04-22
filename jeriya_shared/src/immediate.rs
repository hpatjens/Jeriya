use std::sync::Arc;

use nalgebra::{Vector3, Vector4};

use crate::{backend::Backend, AsDebugInfo, DebugInfo, ImmediateCommandBufferBuilder};

/// Configuration for immediate line rendering
#[derive(Debug, Clone)]
pub struct LineConfig {
    pub color: Vector4<f32>,
    pub line_width: f32,
}

impl Default for LineConfig {
    fn default() -> Self {
        Self {
            color: Vector4::new(1.0, 1.0, 1.0, 1.0),
            line_width: 1.0,
        }
    }
}

/// Individual lines for immediate rendering
#[derive(Debug, Clone)]
pub struct LineList {
    vertices: Vec<Vector3<f32>>,
    config: LineConfig,
}

impl LineList {
    /// Creates a new `LineList` from the given vertices
    ///
    /// # Panics
    ///
    /// - Panics if the number of vertices is not even.
    pub fn new(vertices: Vec<Vector3<f32>>, config: LineConfig) -> Self {
        assert!(vertices.len() % 2 == 0, "Number of vertices must be even");
        Self { vertices, config }
    }

    /// Returns the vertices of the `LineList`
    pub fn vertices(&self) -> &[Vector3<f32>] {
        &self.vertices
    }

    /// Returns the `LineConfig` of the `LineList`
    pub fn config(&self) -> &LineConfig {
        &self.config
    }
}

/// Line strip for immediate rendering
#[derive(Debug, Clone)]
pub struct LineStrip {
    vertices: Vec<Vector3<f32>>,
    config: LineConfig,
}

impl LineStrip {
    /// Creates a new `LineStrip` from the given vertices
    pub fn new(vertices: Vec<Vector3<f32>>, config: LineConfig) -> Self {
        assert!(!vertices.is_empty(), "Number of vertices must be greater than 0");
        Self { vertices, config }
    }

    /// Returns the vertices of the `LineStrip`
    pub fn vertices(&self) -> &[Vector3<f32>] {
        &self.vertices
    }

    /// Returns the `LineConfig` of the `LineStrip`
    pub fn config(&self) -> &LineConfig {
        &self.config
    }
}

/// Command buffer for immediate rendering.
pub struct CommandBuffer<B: Backend> {
    command_buffer: B::ImmediateCommandBuffer,
}

impl<B: Backend> CommandBuffer<B> {
    /// Creates a new `CommandBuffer` from the given `command_buffer`.
    pub fn new(command_buffer: B::ImmediateCommandBuffer) -> Self {
        Self { command_buffer }
    }

    /// Returns the underlying command buffer.
    pub fn command_buffer(&self) -> &B::ImmediateCommandBuffer {
        &self.command_buffer
    }
}

impl<B: Backend> AsDebugInfo for CommandBuffer<B> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.command_buffer.as_debug_info()
    }
}

/// Creates new command buffer in the [`ImmediateRenderingBackend`].
pub struct CommandBufferBuilder<B: Backend> {
    command_buffer_builder: B::ImmediateCommandBufferBuilder,
}

impl<B: Backend> CommandBufferBuilder<B> {
    /// Creates a new `CommandBufferBuilder`.
    pub fn new(command_buffer_builder: B::ImmediateCommandBufferBuilder) -> Self {
        Self { command_buffer_builder }
    }

    /// Pushes new [`LineList`]s to the `CommandBufferBuilder`.
    pub fn push_line_lists(mut self, lines: &[LineList]) -> crate::Result<Self> {
        self.command_buffer_builder.push_line_lists(lines)?;
        Ok(self)
    }

    /// Pushes new [`LineStrip`]s to the `CommandBufferBuilder`.
    pub fn push_line_strips(mut self, line_strip: &[LineStrip]) -> crate::Result<Self> {
        self.command_buffer_builder.push_line_strips(line_strip)?;
        Ok(self)
    }

    /// Finalizes the creation of the [`CommandBuffer`].
    pub fn build(self) -> crate::Result<Arc<CommandBuffer<B>>> {
        self.command_buffer_builder.build()
    }
}

impl<B: Backend> AsDebugInfo for CommandBufferBuilder<B> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.command_buffer_builder.as_debug_info()
    }
}
