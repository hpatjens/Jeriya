use crate::Resource;

#[derive(Default)]
pub struct Texture2d;

impl Resource for Texture2d {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }
}

impl Texture2d {
    pub fn width(&self) -> u32 {
        0
    }

    pub fn height(&self) -> u32 {
        0
    }
}
