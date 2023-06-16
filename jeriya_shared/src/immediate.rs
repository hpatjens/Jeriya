use std::sync::Arc;

use nalgebra::{Matrix4, Vector3, Vector4};

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
    positions: Vec<Vector3<f32>>,
    config: LineConfig,
}

impl LineList {
    /// Creates a new `LineList` from the given positions
    ///
    /// # Panics
    ///
    /// - Panics if the number of positions is not even.
    pub fn new(positions: Vec<Vector3<f32>>, config: LineConfig) -> Self {
        assert!(positions.len() % 2 == 0, "Number of vertices must be even");
        Self { positions, config }
    }

    /// Returns the vertices of the `LineList`
    pub fn positions(&self) -> &[Vector3<f32>] {
        &self.positions
    }

    /// Returns the [`LineConfig`] of the `LineList`
    pub fn config(&self) -> &LineConfig {
        &self.config
    }
}

/// Line strip for immediate rendering
#[derive(Debug, Clone)]
pub struct LineStrip {
    positions: Vec<Vector3<f32>>,
    config: LineConfig,
}

impl LineStrip {
    /// Creates a new `LineStrip` from the given positions
    pub fn new(positions: Vec<Vector3<f32>>, config: LineConfig) -> Self {
        Self { positions, config }
    }

    /// Returns the positions of the `LineStrip`
    pub fn positions(&self) -> &[Vector3<f32>] {
        &self.positions
    }

    /// Returns the [`LineConfig`] of the `LineStrip`
    pub fn config(&self) -> &LineConfig {
        &self.config
    }
}

/// Configuration for immediate triangle rendering
#[derive(Debug, Clone)]
pub struct TriangleConfig {
    pub color: Vector4<f32>,
}

impl Default for TriangleConfig {
    fn default() -> Self {
        Self {
            color: Vector4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// Individual triangles for immediate rendering
#[derive(Debug, Clone)]
pub struct TriangleList {
    positions: Vec<Vector3<f32>>,
    config: TriangleConfig,
}

impl TriangleList {
    /// Creates a new `TriangleList` from the given positions
    ///
    /// # Panics
    ///
    /// - Panics if the number of positions is not a multiple of 3.
    pub fn new(positions: Vec<Vector3<f32>>, config: TriangleConfig) -> Self {
        assert!(positions.len() % 3 == 0, "Number of vertices must be a multiple of 3");
        Self { positions, config }
    }

    /// Returns the positions of the `LineStrip`
    pub fn positions(&self) -> &[Vector3<f32>] {
        &self.positions
    }

    /// Returns the [`TriangleConfig`] of the `TriangleStrip`
    pub fn config(&self) -> &TriangleConfig {
        &self.config
    }
}

/// Triangle strip for immediate rendering
#[derive(Debug, Clone)]
pub struct TriangleStrip {
    positions: Vec<Vector3<f32>>,
    config: TriangleConfig,
}

impl TriangleStrip {
    /// Creates a new `TriangleStrip` from the given positions
    pub fn new(positions: Vec<Vector3<f32>>, config: TriangleConfig) -> Self {
        Self { positions, config }
    }

    /// Returns the positions of the `TriangleStrip`
    pub fn positions(&self) -> &[Vector3<f32>] {
        &self.positions
    }

    /// Returns the [`TriangleConfig`] of the `TriangleStrip`
    pub fn config(&self) -> &TriangleConfig {
        &self.config
    }
}

/// Command buffer for immediate rendering.
pub struct CommandBuffer<B: Backend> {
    command_buffer: B::ImmediateCommandBufferHandler,
}

impl<B: Backend> CommandBuffer<B> {
    /// Creates a new `CommandBuffer` from the given `command_buffer`.
    pub fn new(command_buffer: B::ImmediateCommandBufferHandler) -> Self {
        Self { command_buffer }
    }

    /// Returns the underlying command buffer.
    pub fn command_buffer(&self) -> &B::ImmediateCommandBufferHandler {
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
    command_buffer_builder: B::ImmediateCommandBufferBuilderHandler,
}

impl<B: Backend> CommandBufferBuilder<B> {
    /// Creates a new `CommandBufferBuilder`.
    pub fn new(command_buffer_builder: B::ImmediateCommandBufferBuilderHandler) -> Self {
        Self { command_buffer_builder }
    }

    /// Sets the matrix to be used for the following draw calls.
    pub fn matrix(mut self, matrix: Matrix4<f32>) -> crate::Result<Self> {
        self.command_buffer_builder.matrix(matrix)?;
        Ok(self)
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

    /// Pushes new [`TriangleList`]s to the `CommandBufferBuilder`.
    pub fn push_triangle_lists(mut self, triangle_lists: &[TriangleList]) -> crate::Result<Self> {
        self.command_buffer_builder.push_triangle_lists(triangle_lists)?;
        Ok(self)
    }

    /// Pushes new [`TriangleStrip`]s to the `CommandBufferBuilder`
    pub fn push_triangle_strips(mut self, triangle_strip: &[TriangleStrip]) -> crate::Result<Self> {
        self.command_buffer_builder.push_triangle_strips(triangle_strip)?;
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
