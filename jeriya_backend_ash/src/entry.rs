use std::sync::Arc;

use crate::{Error, RawVulkan};

pub struct Entry {
    entry: ash::Entry,
}

impl RawVulkan for Entry {
    type Output = ash::Entry;
    fn raw_vulkan(&self) -> &Self::Output {
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
