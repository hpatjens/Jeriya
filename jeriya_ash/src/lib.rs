use jeriya::Backend;

pub struct Ash {}

impl Backend for Ash {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }
}
