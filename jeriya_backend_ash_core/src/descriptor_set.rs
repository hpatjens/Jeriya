use std::sync::Arc;

use ash::vk;
use jeriya_shared::{parking_lot::Mutex, DebugInfo};

use crate::{descriptor_pool::DescriptorPool, descriptor_set_layout::DescriptorSetLayout, device::Device, AsRawVulkan};

pub struct DescriptorSet {
    device: Arc<Device>,
    descriptor_pool: Arc<Mutex<DescriptorPool>>,
    _descriptor_set_layout: Arc<DescriptorSetLayout>,
    descriptor_set: vk::DescriptorSet,
    debug_info: DebugInfo,
}

impl Drop for DescriptorSet {
    fn drop(&mut self) {
        let descriptor_pool = self.descriptor_pool.lock();
        unsafe {
            if let Err(e) = self
                .device
                .as_raw_vulkan()
                .free_descriptor_sets(*descriptor_pool.as_raw_vulkan(), &[self.descriptor_set])
            {
                panic!("Failed to free descriptor set {}: {e}", self.debug_info.format_one_line());
            }
        }
    }
}

impl DescriptorSet {
    /// Allocates a new [`DescriptorSet`] from the given [`DescriptorPool`]
    pub fn allocate(
        device: &Arc<Device>,
        descriptor_pool: &Arc<Mutex<DescriptorPool>>,
        descriptor_set_layout: &Arc<DescriptorSetLayout>,
        debug_info: DebugInfo,
    ) -> crate::Result<DescriptorSet> {
        DescriptorPool::allocate_descriptor_set(device, descriptor_pool, descriptor_set_layout, debug_info)
    }

    /// Returns a new [`DescriptorSet`] that was previously allocated from the given [`DescriptorPool`] and is based on the given [`DescriptorSetLayout`]
    pub(crate) fn new_allocated(
        device: Arc<Device>,
        descriptor_pool: Arc<Mutex<DescriptorPool>>,
        descriptor_set_layout: Arc<DescriptorSetLayout>,
        descriptor_set: vk::DescriptorSet,
        debug_info: DebugInfo,
    ) -> Self {
        Self {
            device,
            descriptor_pool,
            _descriptor_set_layout: descriptor_set_layout,
            descriptor_set,
            debug_info,
        }
    }
}

impl AsRawVulkan for DescriptorSet {
    type Output = vk::DescriptorSet;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.descriptor_set
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::sync::Arc;

        use jeriya_shared::{debug_info, parking_lot::Mutex};

        use crate::{
            descriptor_pool::DescriptorPool, descriptor_set::DescriptorSet, descriptor_set_layout::DescriptorSetLayout,
            device::tests::TestFixtureDevice, swapchain::Swapchain,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let swapchain = Swapchain::new(&test_fixture_device.device, &test_fixture_device.surface, 2, None).unwrap();
            let descriptor_set_layout = DescriptorSetLayout::builder()
                .push_uniform_buffer::<f32>(0, 1)
                .build(&test_fixture_device.device)
                .unwrap();
            let descriptor_pool = DescriptorPool::builder(&test_fixture_device.device, &swapchain)
                .make_space_for(&descriptor_set_layout, 1)
                .build()
                .unwrap();
            let _descriptor_set = DescriptorSet::allocate(
                &test_fixture_device.device,
                &Arc::new(Mutex::new(descriptor_pool)),
                &Arc::new(descriptor_set_layout),
                debug_info!("test"),
            );
        }
    }
}
