use std::sync::Arc;

use ash::vk;
use jeriya_backend::gpu_index_allocator::GpuIndexAllocation;
use jeriya_shared::DebugInfo;

use crate::{
    buffer::{Buffer, BufferUsageFlags, GeneralBuffer},
    device::Device,
    host_visible_buffer::HostVisibleBuffer,
    shader_interface::Represents,
    AsRawVulkan,
};

/// A buffer that stores the values that are required per frame.
pub struct FrameLocalBuffer<T> {
    high_water_mark: usize,
    host_visible_buffer: HostVisibleBuffer<T>,
    debug_info: DebugInfo,
}

impl<T> FrameLocalBuffer<T>
where
    T: Default + Clone,
{
    /// Creates a new [`FrameLocalBuffer`] with the given capacity.
    pub fn new(device: &Arc<Device>, capacity: usize, debug_info: DebugInfo) -> crate::Result<Self> {
        let host_visible_buffer = HostVisibleBuffer::new(
            device,
            &vec![T::default(); capacity],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info.clone(),
        )?;
        Ok(Self {
            high_water_mark: 0,
            host_visible_buffer,
            debug_info,
        })
    }

    /// Sets the value at the given index.
    pub fn set<A>(&mut self, gpu_index_allocation: &GpuIndexAllocation<A>, value: &T) -> crate::Result<()>
    where
        T: Represents<A>,
    {
        self.host_visible_buffer
            .set_memory_unaligned_index(gpu_index_allocation.index(), value)?;
        self.high_water_mark = self.high_water_mark.max(gpu_index_allocation.index() + 1);
        Ok(())
    }

    /// Returns the count of used values in the [`FrameLocalBuffer`].
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark
    }

    /// Returns the [`HostVisibleBuffer`] that stores the values of the [`FrameLocalBuffer`].
    #[cfg(test)]
    pub fn host_visible_buffer(&self) -> &HostVisibleBuffer<T> {
        &self.host_visible_buffer
    }

    /// Returns the capacity of the [`FrameLocalBuffer`].
    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.host_visible_buffer.len()
    }

    /// Returns the [`DebugInfo`] of the [`FrameLocalBuffer`].
    #[allow(dead_code)]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl<T> AsRawVulkan for FrameLocalBuffer<T> {
    type Output = vk::Buffer;
    fn as_raw_vulkan(&self) -> &Self::Output {
        self.host_visible_buffer.as_raw_vulkan()
    }
}

impl<T> GeneralBuffer for FrameLocalBuffer<T> {}
impl<T> Buffer<T> for FrameLocalBuffer<T> {}

#[cfg(test)]
mod tests {
    use jeriya_shared::debug_info;

    use super::*;

    use crate::device::TestFixtureDevice;

    #[test]
    fn smoke() {
        #[derive(Default, Clone)]
        struct GpuType(u32);
        struct CpuType(u32);
        impl Represents<CpuType> for GpuType {}

        let device_test_fixture = TestFixtureDevice::new().unwrap();
        let mut frame_local_buffer = FrameLocalBuffer::<GpuType>::new(&device_test_fixture.device, 10, debug_info!("my_buffer")).unwrap();
        assert_eq!(frame_local_buffer.capacity(), 10);
        assert_eq!(frame_local_buffer.high_water_mark(), 0);
        assert_eq!(frame_local_buffer.host_visible_buffer().len(), 10);
        assert_eq!(frame_local_buffer.debug_info().name(), "my_buffer");

        let gpu_index_allocation = GpuIndexAllocation::<CpuType>::new_unchecked(0);
        frame_local_buffer.set(&gpu_index_allocation, &GpuType(73)).unwrap();
    }
}
