use std::sync::Arc;

use ash::vk;

use crate::{device::Device, AsRawVulkan};

pub struct Semaphore {
    semaphore: vk::Semaphore,
    device: Arc<Device>,
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe { self.device.as_raw_vulkan().destroy_semaphore(self.semaphore, None) }
    }
}

impl Semaphore {
    pub fn new(device: &Arc<Device>) -> crate::Result<Self> {
        Ok(Self {
            semaphore: unsafe { device.as_raw_vulkan().create_semaphore(&Default::default(), None)? },
            device: device.clone(),
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
        use crate::{device::tests::TestFixtureDevice, semaphore::Semaphore};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let _semaphore = Semaphore::new(&test_fixture_device.device).unwrap();
        }
    }
}
