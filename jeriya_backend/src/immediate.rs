use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use jeriya_shared::{
    nalgebra::{Matrix4, Vector3, Vector4},
    AsDebugInfo, DebugInfo,
};

/// Identifies a frame for immediate rendering.
///
/// Create an `ImmediateRenderingFrame` with [`ImmediateRenderingFrame::new`] once per frame
/// and pass it to [`Renderer::render_immediate_command_buffer`]. The `ImmediateRenderingFrame`
/// will determine how long the [`ImmediateCommandBuffer`] will be rendered.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImmediateRenderingFrame(Arc<InnerImmediateRenderingFrame>);

impl ImmediateRenderingFrame {
    /// Creates a new `ImmediateRenderingFrame` with the given `update_loop_name`, `index` and `timeout`. This method allocates memory and should only be called once per frame.
    pub fn new(update_loop_name: &'static str, index: u64, timeout: Timeout) -> Self {
        Self(Arc::new(InnerImmediateRenderingFrame {
            update_loop_name,
            index,
            timeout,
        }))
    }

    /// Returns the timeout of the `ImmediateRenderingFrame`.
    pub fn timeout(&self) -> &Timeout {
        &self.0.timeout
    }

    /// Name of the update loop from which the `ImmediateRenderingFrame` was created.
    pub fn update_loop_name(&self) -> &'static str {
        self.0.update_loop_name
    }

    /// Index of the `ImmediateRenderingFrame`.
    pub fn index(&self) -> u64 {
        self.0.index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InnerImmediateRenderingFrame {
    update_loop_name: &'static str,
    index: u64,
    timeout: Timeout,
}

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

/// Timeout for an [`ImmediateRenderingFrame`]
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Timeout {
    /// The frame will be rendered until a new frame is requested.
    #[default]
    Infinite,
    /// The frame will be rendered no longer than the given [`Duration`].
    Finite(Duration),
}

impl Timeout {
    /// Returns `true` if the given `start_time` is timed out.
    pub fn is_timed_out(&self, start_time: &Instant) -> bool {
        match self {
            Timeout::Infinite => false,
            Timeout::Finite(duration) => start_time.elapsed() > *duration,
        }
    }
}

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

#[derive(Debug, Clone)]
pub enum ImmediateCommand {
    Matrix(Matrix4<f32>),
    LineList(LineList),
    LineStrip(LineStrip),
    TriangleList(TriangleList),
    TriangleStrip(TriangleStrip),
}

/// Command buffer for immediate rendering.
#[derive(Clone)]
pub struct CommandBuffer {
    commands: Vec<ImmediateCommand>,
    debug_info: DebugInfo,
}

impl CommandBuffer {
    /// Creates a new `CommandBuffer` from the given `command_buffer`.
    pub fn new(debug_info: DebugInfo) -> Self {
        Self {
            commands: Vec::new(),
            debug_info,
        }
    }

    pub fn commands(&self) -> &[ImmediateCommand] {
        &self.commands
    }
}

impl AsDebugInfo for CommandBuffer {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

/// Creates new command buffer in the [`ImmediateRenderingBackend`].
pub struct CommandBufferBuilder {
    command_buffer: CommandBuffer,
}

impl CommandBufferBuilder {
    /// Creates a new `CommandBufferBuilder`.
    pub fn new(debug_info: DebugInfo) -> Self {
        Self {
            command_buffer: CommandBuffer::new(debug_info),
        }
    }

    /// Sets the matrix to be used for the following draw calls.
    pub fn matrix(mut self, matrix: Matrix4<f32>) -> crate::Result<Self> {
        self.command_buffer.commands.push(ImmediateCommand::Matrix(matrix));
        Ok(self)
    }

    /// Pushes new [`LineList`]s to the `CommandBufferBuilder`.
    pub fn push_line_lists(mut self, line_lists: &[LineList]) -> crate::Result<Self> {
        for line_list in line_lists {
            if line_list.positions().is_empty() {
                continue;
            }
            self.command_buffer.commands.push(ImmediateCommand::LineList(line_list.clone()));
        }
        Ok(self)
    }

    /// Pushes new [`LineStrip`]s to the `CommandBufferBuilder`.
    pub fn push_line_strips(mut self, line_strips: &[LineStrip]) -> crate::Result<Self> {
        for line_strip in line_strips {
            if line_strip.positions().is_empty() {
                continue;
            }
            self.command_buffer.commands.push(ImmediateCommand::LineStrip(line_strip.clone()));
        }
        Ok(self)
    }

    /// Pushes new [`TriangleList`]s to the `CommandBufferBuilder`.
    pub fn push_triangle_lists(mut self, triangle_lists: &[TriangleList]) -> crate::Result<Self> {
        for triangle_list in triangle_lists {
            if triangle_list.positions().is_empty() {
                continue;
            }
            self.command_buffer
                .commands
                .push(ImmediateCommand::TriangleList(triangle_list.clone()));
        }
        Ok(self)
    }

    /// Pushes new [`TriangleStrip`]s to the `CommandBufferBuilder`
    pub fn push_triangle_strips(mut self, triangle_strips: &[TriangleStrip]) -> crate::Result<Self> {
        for triangle_strip in triangle_strips {
            if triangle_strip.positions().is_empty() {
                continue;
            }
            self.command_buffer
                .commands
                .push(ImmediateCommand::TriangleStrip(triangle_strip.clone()));
        }
        Ok(self)
    }

    /// Finalizes the creation of the [`CommandBuffer`].
    pub fn build(self) -> crate::Result<CommandBuffer> {
        Ok(self.command_buffer)
    }
}

impl AsDebugInfo for CommandBufferBuilder {
    fn as_debug_info(&self) -> &DebugInfo {
        self.command_buffer.as_debug_info()
    }
}
