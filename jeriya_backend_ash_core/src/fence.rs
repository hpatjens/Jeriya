use std::sync::Arc;

use ash::vk;

use crate::{device::Device, AsRawVulkan};

pub struct Fence {
    fence: vk::Fence,
    device: Arc<Device>,
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            self.device.as_raw_vulkan().destroy_fence(self.fence, None);
        }
    }
}

impl Fence {
    pub fn new(device: &Arc<Device>) -> crate::Result<Self> {
        let fence_create_info = vk::FenceCreateInfo::default();
        let fence = unsafe { device.as_raw_vulkan().create_fence(&fence_create_info, None)? };
        Ok(Self {
            fence,
            device: device.clone(),
        })
    }

    /// Queries the state of the fence
    pub fn get_fence_status(&self) -> crate::Result<bool> {
        unsafe { Ok(self.device.as_raw_vulkan().get_fence_status(self.fence)?) }
    }

    /// Waits until the fence gets signalled
    pub fn wait(&self) -> crate::Result<()> {
        unsafe { Ok(self.device.as_raw_vulkan().wait_for_fences(&[self.fence], true, u64::MAX)?) }
    }
}

impl AsRawVulkan for Fence {
    type Output = vk::Fence;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.fence
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use crate::{device::tests::TestFixtureDevice, fence::Fence};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let _fence = Fence::new(&test_fixture_device.device).unwrap();
        }
    }
}
