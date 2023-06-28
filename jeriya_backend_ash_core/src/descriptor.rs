use std::any::TypeId;

use ash::vk;
use jeriya_shared::derive_more::Constructor;

#[derive(Constructor, Clone, Debug)]
pub struct Descriptor {
    pub binding: u32,
    pub descriptor_type: DescriptorType,
    pub descriptor_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DescriptorType {
    UniformBuffer(TypeId),
    StorageBuffer(TypeId),
}

impl DescriptorType {
    pub fn new_uniform_buffer<T: 'static>() -> Self {
        Self::UniformBuffer(TypeId::of::<T>())
    }

    pub fn new_storage_buffer<T: 'static>() -> Self {
        Self::StorageBuffer(TypeId::of::<T>())
    }
}

impl From<DescriptorType> for vk::DescriptorType {
    fn from(descriptor_type: DescriptorType) -> Self {
        match descriptor_type {
            DescriptorType::UniformBuffer(_) => vk::DescriptorType::UNIFORM_BUFFER,
            DescriptorType::StorageBuffer(_) => vk::DescriptorType::STORAGE_BUFFER,
        }
    }
}
