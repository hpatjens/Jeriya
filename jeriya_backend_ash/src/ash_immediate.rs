use std::sync::Arc;

use jeriya_backend::{
    immediate::{self, LineList, LineStrip, TriangleList, TriangleStrip},
    ImmediateCommandBufferBuilderHandler,
};
use jeriya_shared::{nalgebra::Matrix4, AsDebugInfo, DebugInfo};

use crate::AshBackend;

#[derive(Debug, Clone)]
pub(crate) enum ImmediateCommand {
    Matrix(Matrix4<f32>),
    LineList(LineList),
    LineStrip(LineStrip),
    TriangleList(TriangleList),
    TriangleStrip(TriangleStrip),
}

#[derive(Debug)]
pub struct AshImmediateCommandBufferHandler {
    pub(crate) commands: Vec<ImmediateCommand>,
    pub(crate) debug_info: DebugInfo,
}

impl AsDebugInfo for AshImmediateCommandBufferHandler {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

pub struct AshImmediateCommandBufferBuilderHandler {
    commands: Vec<ImmediateCommand>,
    debug_info: DebugInfo,
}

impl ImmediateCommandBufferBuilderHandler for AshImmediateCommandBufferBuilderHandler {
    type Backend = AshBackend;

    fn new(_backend: &Self::Backend, debug_info: DebugInfo) -> jeriya_backend::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            commands: Vec::new(),
            debug_info,
        })
    }

    fn matrix(&mut self, matrix: Matrix4<f32>) -> jeriya_backend::Result<()> {
        self.commands.push(ImmediateCommand::Matrix(matrix));
        Ok(())
    }

    fn push_line_lists(&mut self, line_lists: &[LineList]) -> jeriya_backend::Result<()> {
        for line_list in line_lists {
            if line_list.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::LineList(line_list.clone()));
        }
        Ok(())
    }

    fn push_line_strips(&mut self, line_strips: &[LineStrip]) -> jeriya_backend::Result<()> {
        for line_strip in line_strips {
            if line_strip.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::LineStrip(line_strip.clone()));
        }
        Ok(())
    }

    fn push_triangle_lists(&mut self, triangle_lists: &[TriangleList]) -> jeriya_backend::Result<()> {
        for triangle_list in triangle_lists {
            if triangle_list.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::TriangleList(triangle_list.clone()));
        }
        Ok(())
    }

    fn push_triangle_strips(&mut self, triangle_strips: &[TriangleStrip]) -> jeriya_backend::Result<()> {
        for triangle_strip in triangle_strips {
            if triangle_strip.positions().is_empty() {
                continue;
            }
            self.commands.push(ImmediateCommand::TriangleStrip(triangle_strip.clone()));
        }
        Ok(())
    }

    fn build(self) -> jeriya_backend::Result<Arc<immediate::CommandBuffer<Self::Backend>>> {
        let command_buffer = AshImmediateCommandBufferHandler {
            commands: self.commands,
            debug_info: self.debug_info,
        };
        Ok(Arc::new(immediate::CommandBuffer::new(command_buffer)))
    }
}

impl AsDebugInfo for AshImmediateCommandBufferBuilderHandler {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}
