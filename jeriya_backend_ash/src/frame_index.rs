#[derive(Default, Debug, Clone)]
pub struct FrameIndex {
    /// Index of the frame in the lifetime of the application
    index: u64,
    /// Index of the swapchain image (None before it is determined in the frame)
    swapchain_index: Option<usize>,
}

impl FrameIndex {
    /// Create the `FrameIndex` for the first frame
    pub fn new() -> Self {
        Self {
            index: 0,
            swapchain_index: None,
        }
    }

    /// Increment the frame index
    pub fn increment(&mut self) {
        self.index += 1;
    }

    /// Set the swapchain index
    pub fn set_swapchain_index(&mut self, swapchain_index: impl Into<Option<usize>>) {
        self.swapchain_index = swapchain_index.into();
    }

    /// Get the frame index
    pub fn index(&self) -> u64 {
        self.index
    }

    /// Get the swapchain index
    pub fn swapchain_index(&self) -> Option<usize> {
        self.swapchain_index
    }
}
