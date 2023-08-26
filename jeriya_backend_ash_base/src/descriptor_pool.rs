use std::{collections::BTreeMap, sync::Arc};

use ash::vk;
use jeriya_shared::{parking_lot::Mutex, DebugInfo};

use crate::{
    descriptor::DescriptorType, descriptor_set::DescriptorSet, descriptor_set_layout::DescriptorSetLayout, device::Device,
    swapchain::Swapchain, AsRawVulkan, DebugInfoAshExtension, Error,
};

pub struct DescriptorPool {
    device: Arc<Device>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_type_capacities: BTreeMap<DescriptorType, usize>,
    _descriptor_set_capacities: usize,
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe { self.device.as_raw_vulkan().destroy_descriptor_pool(self.descriptor_pool, None) };
    }
}

impl DescriptorPool {
    pub(crate) fn allocate_descriptor_set(
        device: &Arc<Device>,
        descriptor_pool: &Arc<Mutex<DescriptorPool>>,
        descriptor_set_layout: &Arc<DescriptorSetLayout>,
        debug_info: DebugInfo,
    ) -> crate::Result<DescriptorSet> {
        let descriptor_pool_guard = descriptor_pool.lock();

        // Check if the given descriptor set layout can be allocated from this descriptor pool
        if !descriptor_pool_guard.has_enough_space_for(descriptor_set_layout) {
            return Err(Error::DescriptorPoolDoesntHaveEnoughSpace);
        }

        // Allocate the descriptor set
        let descriptor_set_layouts = [*descriptor_set_layout.as_raw_vulkan()];
        let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: *descriptor_pool_guard.as_raw_vulkan(),
            descriptor_set_count: descriptor_set_layouts.len() as u32,
            p_set_layouts: descriptor_set_layouts.as_ptr(),
            ..Default::default()
        };
        let allocated_descriptor_sets = unsafe {
            device
                .as_raw_vulkan()
                .allocate_descriptor_sets(&descriptor_set_allocate_info)
                .map_err(|_| Error::FailedToAllocate("DescriptorSet"))
        }?;
        let debug_info = debug_info.with_vulkan_ptr(allocated_descriptor_sets[0]);
        Ok(DescriptorSet::new_allocated(
            device.clone(),
            descriptor_pool.clone(),
            descriptor_set_layout.clone(),
            allocated_descriptor_sets[0],
            debug_info,
        ))
    }

    /// Creates a new [`DescriptorPoolBuilder`] to create a [`DescriptorPool`] for all the given [`DescriptorSetLayout`]s
    pub fn builder<'a>(device: &Arc<Device>, swapchain: &'a Swapchain) -> DescriptorPoolBuilder<'a> {
        DescriptorPoolBuilder::new(device.clone(), swapchain)
    }

    /// Checks if the given [`DescriptorSetLayout`] can be allocated from this [`DescriptorPool`]
    pub fn has_enough_space_for(&self, descriptor_set_layout: &DescriptorSetLayout) -> bool {
        for (descriptor_type, descriptors) in descriptor_set_layout.descriptors_by_type() {
            if self.descriptor_type_capacities.get(&descriptor_type).unwrap_or(&0) < &descriptors.len() {
                return false;
            }
        }
        true
    }
}

impl AsRawVulkan for DescriptorPool {
    type Output = vk::DescriptorPool;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.descriptor_pool
    }
}

pub struct DescriptorPoolBuilder<'a> {
    device: Arc<Device>,
    swapchain: &'a Swapchain,
    descriptor_type_capacities: BTreeMap<DescriptorType, usize>,
    descriptor_set_capacities: usize,
}

impl<'a> DescriptorPoolBuilder<'a> {
    /// Creates a new [`DescriptorPoolBuilder`] to create a [`DescriptorPool`] for all images of the given [`Swapchain`]
    pub fn new(device: Arc<Device>, swapchain: &'a Swapchain) -> Self {
        Self {
            device,
            swapchain,
            descriptor_type_capacities: BTreeMap::new(),
            descriptor_set_capacities: 0,
        }
    }

    /// Allocates space the given [`DescriptorSetLayout`]s
    pub fn make_space_for(mut self, descriptor_set_layout: &DescriptorSetLayout, count: usize) -> Self {
        for descriptor in descriptor_set_layout.descriptors().iter() {
            let increase = count * self.swapchain.len() * descriptor.descriptor_count as usize;
            self.descriptor_type_capacities
                .entry(descriptor.descriptor_type)
                .and_modify(|value| *value += increase)
                .or_insert(increase);
        }
        self.descriptor_set_capacities += 1;
        self
    }

    /// Builds the [`DescriptorPool`]
    pub fn build(self) -> crate::Result<DescriptorPool> {
        // Create the vk::DescriptorPoolSizes
        let descriptor_pool_sizes = self
            .descriptor_type_capacities
            .iter()
            .map(|(&ty, descriptor_count)| vk::DescriptorPoolSize {
                ty: ty.into(),
                descriptor_count: *descriptor_count as u32,
            })
            .collect::<Vec<_>>();

        // Create DescriptorPool
        let descriptor_pool = unsafe {
            let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo {
                pool_size_count: descriptor_pool_sizes.len() as u32,
                p_pool_sizes: descriptor_pool_sizes.as_ptr(),
                max_sets: (self.swapchain.len() * self.descriptor_set_capacities) as u32,
                ..Default::default()
            };
            self.device
                .as_raw_vulkan()
                .create_descriptor_pool(&descriptor_pool_create_info, None)
                .unwrap()
        };

        Ok(DescriptorPool {
            device: self.device,
            descriptor_pool,
            descriptor_type_capacities: self.descriptor_type_capacities,
            _descriptor_set_capacities: self.descriptor_set_capacities,
        })
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use crate::{
            descriptor_pool::DescriptorPool, descriptor_set_layout::DescriptorSetLayout, device::tests::TestFixtureDevice,
            swapchain::Swapchain,
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
            assert!(descriptor_pool.has_enough_space_for(&descriptor_set_layout));
        }
    }
}
