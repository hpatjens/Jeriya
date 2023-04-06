use std::sync::Arc;

use ash::vk;

use crate::{device::Device, swapchain::Swapchain, swapchain_vec::SwapchainVec, AsRawVulkan};

/// Depth Buffer for the Swapchain
pub struct SwapchainDepthBuffers {
    pub depth_buffers: SwapchainVec<SwapchainDepthBuffer>,
}

impl SwapchainDepthBuffers {
    /// Creates a new depth buffer for the given [`Swapchain`]
    pub fn new(device: &Arc<Device>, swapchain: &Swapchain) -> crate::Result<Self> {
        let depth_buffers = SwapchainVec::new(swapchain, |_| SwapchainDepthBuffer::new(device, swapchain))?;
        Ok(Self { depth_buffers })
    }
}

#[non_exhaustive]
pub struct SwapchainDepthBuffer {
    pub depth_image: vk::Image,
    pub depth_image_memory: vk::DeviceMemory,
    pub depth_image_view: vk::ImageView,
    device: Arc<Device>,
}

impl Drop for SwapchainDepthBuffer {
    fn drop(&mut self) {
        unsafe {
            let device = self.device.as_raw_vulkan();
            device.destroy_image_view(self.depth_image_view, None);
            device.free_memory(self.depth_image_memory, None);
            device.destroy_image(self.depth_image, None);
        }
    }
}

impl SwapchainDepthBuffer {
    fn new(device: &Arc<Device>, swapchain: &Swapchain) -> crate::Result<Self> {
        // Image
        let format = vk::Format::D24_UNORM_S8_UINT;
        let depth_image = {
            let depth_image_create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D {
                    width: swapchain.extent().width,
                    height: swapchain.extent().height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            unsafe { device.as_raw_vulkan().create_image(&depth_image_create_info, None)? }
        };

        // Image Memory
        let depth_image_memory = {
            let depth_image_memory_requirements = unsafe { device.as_raw_vulkan().get_image_memory_requirements(depth_image) };
            let depth_image_memory_index = device
                .find_memorytype_index(&depth_image_memory_requirements, vk::MemoryPropertyFlags::DEVICE_LOCAL)
                .ok_or_else(|| crate::Error::UnsupportedMemoryType(depth_image_memory_requirements))?;
            let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(depth_image_memory_requirements.size)
                .memory_type_index(depth_image_memory_index);
            let depth_image_memory = unsafe { device.as_raw_vulkan().allocate_memory(&depth_image_allocate_info, None)? };
            unsafe {
                device.as_raw_vulkan().bind_image_memory(depth_image, depth_image_memory, 0)?;
            }
            depth_image_memory
        };

        // Image View
        let depth_image_view_info = vk::ImageViewCreateInfo::builder()
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .level_count(1)
                    .layer_count(1)
                    .build(),
            )
            .image(depth_image)
            .format(format)
            .view_type(vk::ImageViewType::TYPE_2D);
        let depth_image_view = unsafe { device.as_raw_vulkan().create_image_view(&depth_image_view_info, None)? };

        Ok(Self {
            depth_image,
            depth_image_memory,
            depth_image_view,
            device: device.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use jeriya_test::create_window;

    use crate::{
        device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface, swapchain::Swapchain,
    };

    use super::SwapchainDepthBuffer;

    #[test]
    fn smoke() {
        let window = create_window();
        let entry = Entry::new().unwrap();
        let instance = Instance::new(&entry, "my_application", false).unwrap();
        let surface = Surface::new(&entry, &instance, &window).unwrap();
        let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
        let device = Device::new(physical_device, &instance).unwrap();
        let swapchain = Swapchain::new(&device, &surface, 2, None).unwrap();
        let _swapchain_depthbuffer = SwapchainDepthBuffer::new(&device, &swapchain).unwrap();
    }
}
