use crate::{AsDebugInfo, DebugInfo};

#[derive(Default)]
pub struct Texture2d {
    debug_info: DebugInfo,
}

impl Texture2d {
    pub fn width(&self) -> u32 {
        0
    }

    pub fn height(&self) -> u32 {
        0
    }
}

impl AsDebugInfo for Texture2d {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}
