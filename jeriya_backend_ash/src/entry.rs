use std::sync::Arc;

use crate::{AsRawVulkan, Error};

pub struct Entry {
    entry: ash::Entry,
}

impl AsRawVulkan for Entry {
    type Output = ash::Entry;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.entry
    }
}

impl Entry {
    /// Creates a new `Entry`.
    pub fn new() -> crate::Result<Arc<Self>> {
        let entry = unsafe { ash::Entry::load().map_err(Error::LoadingError)? };
        Ok(Arc::new(Entry { entry }))
    }
}
