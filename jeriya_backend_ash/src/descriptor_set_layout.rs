use std::{any::TypeId, collections::BTreeMap, sync::Arc};

use ash::vk::{self};

use crate::{
    descriptor::{Descriptor, DescriptorType},
    device::Device,
    AsRawVulkan,
};

pub struct DescriptorSetLayout {
    descriptors: Vec<Descriptor>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    device: Arc<Device>,
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        unsafe {
            self.device
                .as_raw_vulkan()
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None)
        }
    }
}

impl DescriptorSetLayout {
    /// Creates a new `DescriptorSetLayout` from the given [`Descriptor`]s
    fn new(device: &Arc<Device>, descriptors: Vec<Descriptor>) -> crate::Result<Self> {
        let descriptor_set_layout_bindings = descriptors
            .iter()
            .map(|descriptor| vk::DescriptorSetLayoutBinding {
                binding: descriptor.binding,
                descriptor_type: descriptor.descriptor_type.into(),
                descriptor_count: descriptor.descriptor_count,
                stage_flags: vk::ShaderStageFlags::ALL_GRAPHICS | vk::ShaderStageFlags::COMPUTE,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo {
            binding_count: descriptor_set_layout_bindings.len() as u32,
            p_bindings: descriptor_set_layout_bindings.as_ptr(),
            flags: vk::DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR,
            ..Default::default()
        };
        let descriptor_set_layout = unsafe {
            device
                .as_raw_vulkan()
                .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)?
        };
        Ok(Self {
            descriptors,
            descriptor_set_layout,
            device: device.clone(),
        })
    }

    /// Creates a new [`DescriptorSetLayoutBuilder`]
    pub fn builder() -> DescriptorSetLayoutBuilder {
        DescriptorSetLayoutBuilder::default()
    }

    /// Returns the [`Descriptor`]s of the `DescriptorSetLayout`
    pub fn descriptors(&self) -> &[Descriptor] {
        &self.descriptors
    }

    /// Returns all [`Descriptor`]s of the `DescriptorSetLayout` grouped by their [`DescriptorType`]s
    pub fn descriptors_by_type(&self) -> BTreeMap<DescriptorType, Vec<Descriptor>> {
        let mut descriptors_by_type = BTreeMap::new();
        for descriptor in &self.descriptors {
            descriptors_by_type
                .entry(descriptor.descriptor_type)
                .or_insert_with(Vec::new)
                .push(descriptor.clone());
        }
        jeriya_shared::assert_eq!(descriptors_by_type.values().map(|v| v.len()).sum::<usize>(), self.descriptors.len());
        descriptors_by_type
    }

    /// Returns all [`Descriptor`]s of the `DescriptorSetLayout` grouped by their [`Descriptor::binding`]s
    pub fn descriptors_by_binding(&self) -> BTreeMap<u32, Descriptor> {
        let mut descriptors_by_binding = BTreeMap::new();
        for descriptor in &self.descriptors {
            jeriya_shared::assert!(!descriptors_by_binding.contains_key(&descriptor.binding), "binding must be unique");
            descriptors_by_binding.insert(descriptor.binding, descriptor.clone());
        }
        jeriya_shared::assert_eq!(descriptors_by_binding.len(), self.descriptors.len());
        descriptors_by_binding
    }
}

impl AsRawVulkan for DescriptorSetLayout {
    type Output = vk::DescriptorSetLayout;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.descriptor_set_layout
    }
}

#[derive(Default)]
pub struct DescriptorSetLayoutBuilder {
    descriptors: Vec<Descriptor>,
}

impl DescriptorSetLayoutBuilder {
    /// Adds a [`Descriptor`] of type uniform buffer to the `DescriptorSetLayout`
    pub fn push_uniform_buffer<T: 'static>(mut self, binding: u32, count: u32) -> Self {
        let ty = DescriptorType::UniformBuffer(TypeId::of::<T>());
        self.descriptors.push(Descriptor::new(binding, ty, count));
        self
    }

    /// Adds a [`Descriptor`] of type storage buffer to the `DescriptorSetLayout`
    pub fn push_storage_buffer<T: 'static>(mut self, binding: u32, count: u32) -> Self {
        let ty = DescriptorType::StorageBuffer(TypeId::of::<T>());
        self.descriptors.push(Descriptor::new(binding, ty, count));
        self
    }

    /// Creates the [`DescriptorSetLayout`] from the given [`Descriptor`]s
    pub fn build(self, device: &Arc<Device>) -> crate::Result<DescriptorSetLayout> {
        DescriptorSetLayout::new(device, self.descriptors)
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use crate::{descriptor_set_layout::DescriptorSetLayout, device::TestFixtureDevice};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let _ = DescriptorSetLayout::builder()
                .push_uniform_buffer::<f32>(0, 1)
                .push_storage_buffer::<u32>(1, 1)
                .build(&test_fixture_device.device)
                .unwrap();
        }
    }
}
