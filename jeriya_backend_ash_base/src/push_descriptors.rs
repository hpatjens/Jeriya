use ash::vk::{self};
use jeriya_shared::bumpalo::Bump;

use crate::{descriptor::DescriptorType, descriptor_set_layout::DescriptorSetLayout, host_visible_buffer::HostVisibleBuffer, AsRawVulkan};

pub struct PushDescriptorBuilder<'a> {
    descriptor_set: &'a DescriptorSetLayout,
    write_descriptor_sets: Vec<vk::WriteDescriptorSet>,
    allocator: Bump,
}

impl<'a> PushDescriptorBuilder<'a> {
    /// Checks if the `DescriptorSetLayout` contains a [`Descriptor`] with the given [`Descriptor::binding`] and [`Descriptor::descriptor_type`]
    fn contains_binding(&self, destination_binding: u32, descriptor_type: DescriptorType) -> bool {
        self.descriptor_set
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.binding == destination_binding && descriptor_type == descriptor.descriptor_type)
    }

    /// Creates a `vk::WriteDescriptorSet` for a `vk::DescriptorType::UNIFORM_BUFFER`
    pub fn push_uniform_buffer<T: 'static>(mut self, destination_binding: u32, host_visible_buffer: &HostVisibleBuffer<T>) -> Self {
        assert!(self.contains_binding(destination_binding, DescriptorType::new_uniform_buffer::<T>()));
        // Must be allocated in an allocator until the write descriptor set is submitted
        let buffer_info = self.allocator.alloc(vk::DescriptorBufferInfo {
            buffer: *host_visible_buffer.as_raw_vulkan(),
            offset: 0,
            range: vk::WHOLE_SIZE,
        });
        let write_descriptor_set = vk::WriteDescriptorSet {
            // Not used for push descriptors
            dst_set: vk::DescriptorSet::null(),
            dst_binding: destination_binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            p_buffer_info: buffer_info as *const _,
            ..Default::default()
        };
        self.write_descriptor_sets.push(write_descriptor_set);
        self
    }

    /// Creates a `vk::WriteDescriptorSet` for a `vk::DescriptorType::STORAGE_BUFFER`
    pub fn push_storage_buffer<T: 'static>(mut self, destination_binding: u32, host_visible_buffer: &HostVisibleBuffer<T>) -> Self {
        assert!(self.contains_binding(destination_binding, DescriptorType::new_storage_buffer::<T>()));
        // Must be allocated in an allocator until the write descriptor set is submitted
        let buffer_info = self.allocator.alloc(vk::DescriptorBufferInfo {
            buffer: *host_visible_buffer.as_raw_vulkan(),
            offset: 0,
            range: vk::WHOLE_SIZE,
        });
        let write_descriptor_set = vk::WriteDescriptorSet {
            // Not used for push descriptors
            dst_set: vk::DescriptorSet::null(),
            dst_binding: destination_binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
            p_buffer_info: buffer_info as *const _,
            ..Default::default()
        };
        self.write_descriptor_sets.push(write_descriptor_set);
        self
    }

    pub fn build(self) -> PushDescriptors {
        PushDescriptors {
            write_descriptor_sets: self.write_descriptor_sets,
            _allocator: self.allocator,
        }
    }
}

pub struct PushDescriptors {
    write_descriptor_sets: Vec<vk::WriteDescriptorSet>,
    _allocator: Bump,
}

impl PushDescriptors {
    pub(crate) fn write_descriptor_sets(&self) -> &[vk::WriteDescriptorSet] {
        &self.write_descriptor_sets
    }

    pub fn builder(descriptor_set: &DescriptorSetLayout) -> PushDescriptorBuilder {
        PushDescriptorBuilder {
            descriptor_set,
            write_descriptor_sets: Vec::new(),
            allocator: Bump::new(),
        }
    }
}
