use std::sync::Arc;

use ash::vk;
use jeriya_shared::{AsDebugInfo, DebugInfo};

use crate::{device::Device, AsRawVulkan, DebugInfoAshExtension};

pub struct Fence {
    fence: vk::Fence,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            self.device.as_raw_vulkan().destroy_fence(self.fence, None);
        }
    }
}

impl AsDebugInfo for Fence {
    fn as_debug_info(&self) -> &jeriya_shared::DebugInfo {
        &self.debug_info
    }
}

impl Fence {
    pub fn new(device: &Arc<Device>, debug_info: DebugInfo) -> crate::Result<Self> {
        let fence_create_info = vk::FenceCreateInfo::default();
        let fence = unsafe { device.as_raw_vulkan().create_fence(&fence_create_info, None)? };
        let debug_info = debug_info.with_vulkan_ptr(fence);
        Ok(Self {
            fence,
            device: device.clone(),
            debug_info,
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
        use jeriya_shared::debug_info;

        use crate::{device::TestFixtureDevice, fence::Fence};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let _fence = Fence::new(&test_fixture_device.device, debug_info!("my_fence")).unwrap();
        }
    }
}
