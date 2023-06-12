use crate::Resource;

#[derive(Default)]
pub struct StaticMesh;

impl Resource for StaticMesh {
    fn new() -> Self
    where
        Self: Sized,
    {
        todo!()
    }
}
