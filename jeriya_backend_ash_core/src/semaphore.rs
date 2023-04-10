use std::sync::Arc;

use ash::vk;
use jeriya_shared::{AsDebugInfo, DebugInfo};

use crate::{device::Device, AsRawVulkan, DebugInfoAshExtension};

pub struct Semaphore {
    semaphore: vk::Semaphore,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe { self.device.as_raw_vulkan().destroy_semaphore(self.semaphore, None) }
    }
}

impl AsDebugInfo for Semaphore {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl Semaphore {
    pub fn new(device: &Arc<Device>, debug_info: DebugInfo) -> crate::Result<Self> {
        let semaphore = unsafe { device.as_raw_vulkan().create_semaphore(&Default::default(), None)? };
        let debug_info = debug_info.with_vulkan_ptr(semaphore);
        Ok(Self {
            semaphore,
            device: device.clone(),
            debug_info,
        })
    }
}

impl AsRawVulkan for Semaphore {
    type Output = vk::Semaphore;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.semaphore
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_shared::debug_info;

        use crate::{device::tests::TestFixtureDevice, semaphore::Semaphore};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let _semaphore = Semaphore::new(&test_fixture_device.device, debug_info!("my_semaphore")).unwrap();
        }
    }
}
