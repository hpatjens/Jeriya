#[derive(Debug, Clone)]
pub struct FrameIndex {
    index: u64,
    swapchain_index: usize,
}

impl FrameIndex {
    pub fn new() -> Self {
        Self {
            index: 0,
            swapchain_index: 0,
        }
    }

    pub fn incremented(&self, swapchain_index: usize) -> Self {
        Self {
            index: self.index + 1,
            swapchain_index,
        }
    }

    pub fn index(&self) -> u64 {
        self.index
    }

    pub fn swapchain_index(&self) -> usize {
        self.swapchain_index
    }
}
