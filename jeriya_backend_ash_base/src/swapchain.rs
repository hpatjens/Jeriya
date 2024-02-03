use ash::{extensions::khr, prelude::VkResult, vk};
use jeriya_shared::log::{info, warn};

use std::{ops::Drop, sync::Arc};

use crate::{device::Device, frame_index::FrameIndex, queue::Queue, semaphore::Semaphore, surface::Surface, AsRawVulkan, Error};

/// Represents the swapchain.
pub struct Swapchain {
    swapchain: khr::Swapchain,
    swapchain_khr: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    _format: vk::SurfaceFormatKHR,
    extent: vk::Extent2D,
    device: Arc<Device>,
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            let device = self.device.as_raw_vulkan();
            device.device_wait_idle().expect("Failed to wait until the device is idle");
            for image_view in &self.image_views {
                device.destroy_image_view(*image_view, None);
            }
            self.swapchain.destroy_swapchain(self.swapchain_khr, None);
        }
    }
}

impl Swapchain {
    /// Creates a new swapchain for the given [`Surface`].
    pub fn new(
        device: &Arc<Device>,
        surface: &Surface,
        desired_swapchain_length: u32,
        previous_swapchain: Option<&Swapchain>,
    ) -> crate::Result<Self> {
        let surface_capabilities = unsafe {
            surface
                .surface
                .get_physical_device_surface_capabilities(*device.physical_device.as_raw_vulkan(), surface.surface_khr)?
        };
        info!("Surface capabilities: {surface_capabilities:?}");

        // Image Count
        let desired_image_count = desired_swapchain_length
            .max(surface_capabilities.min_image_count)
            .min(surface_capabilities.max_image_count);

        // Format
        let format = {
            let surface_formats = unsafe {
                surface
                    .surface
                    .get_physical_device_surface_formats(*device.physical_device.as_raw_vulkan(), surface.surface_khr)?
            };
            let format = surface_formats
                .iter()
                .map(|sfmt| match sfmt.format {
                    vk::Format::UNDEFINED => vk::SurfaceFormatKHR {
                        format: vk::Format::B8G8R8_UNORM,
                        color_space: sfmt.color_space,
                    },
                    _ => *sfmt,
                })
                .next()
                .ok_or(Error::SwapchainSurfaceFormatError)?;
            info!("Format: {format:?}");
            format
        };

        // Extent
        let extent = surface_capabilities.current_extent;
        info!("Swapchain extent: {extent:?}");

        // Swapchain
        let swapchain_loader = khr::Swapchain::new(device.instance().as_raw_vulkan(), device.as_raw_vulkan());
        let swapchain = {
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = unsafe {
                surface
                    .surface
                    .get_physical_device_surface_present_modes(*device.physical_device.as_raw_vulkan(), surface.surface_khr)?
            };
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);
            let mut swapchain_create_info: ash::vk::SwapchainCreateInfoKHRBuilder<'_> = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface.surface_khr)
                .min_image_count(desired_image_count)
                .image_color_space(format.color_space)
                .image_format(format.format)
                .image_extent(extent)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);
            if let Some(previous_swapchain) = previous_swapchain {
                swapchain_create_info = swapchain_create_info.old_swapchain(previous_swapchain.swapchain_khr);
            }
            info!("SwapchainCreateInfoKHR: {:#?}", *swapchain_create_info);
            unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None).unwrap() }
        };

        // Images
        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
        let image_views: Vec<vk::ImageView> = images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::builder()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format.format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::R,
                        g: vk::ComponentSwizzle::G,
                        b: vk::ComponentSwizzle::B,
                        a: vk::ComponentSwizzle::A,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);
                unsafe { device.as_raw_vulkan().create_image_view(&create_view_info, None) }
            })
            .collect::<VkResult<Vec<_>>>()?;

        Ok(Self {
            swapchain: swapchain_loader,
            swapchain_khr: swapchain,
            images,
            image_views,
            _format: format,
            extent,
            device: device.clone(),
        })
    }

    pub fn acquire_next_image(&self, semaphore_to_signal: &Semaphore) -> crate::Result<u32> {
        let _span = jeriya_shared::span!("acquire_next_image");

        let (present_index, is_suboptimal) = unsafe {
            self.swapchain.acquire_next_image(
                self.swapchain_khr,
                std::u64::MAX,
                *semaphore_to_signal.as_raw_vulkan(),
                vk::Fence::null(),
            )?
        };
        if is_suboptimal {
            warn!("Suboptimal swapchain image");
        }
        Ok(present_index)
    }

    pub fn present(
        &self,
        frame_index: &FrameIndex,
        rendering_complete_semaphore: &Semaphore,
        present_queue: &Queue,
    ) -> crate::Result<bool> {
        let _span = jeriya_shared::span!("Swapchain::present");
        let wait_semaphores = [*rendering_complete_semaphore.as_raw_vulkan()];
        let swapchains = [self.swapchain_khr];
        let image_indices = [frame_index.swapchain_index().expect("swapchain image must be set for presenting") as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe { Ok(self.swapchain.queue_present(*present_queue.as_raw_vulkan(), &present_info)?) }
    }

    /// Returns a copy of the `ImageView`s for the swapchain images
    pub fn image_views(&self) -> Vec<vk::ImageView> {
        self.image_views.clone()
    }

    #[allow(dead_code)]
    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    #[allow(dead_code)]
    pub fn surface_format(&self) -> vk::SurfaceFormatKHR {
        self._format
    }

    /// Returns the number of images in the swapchain
    #[allow(clippy::len_without_is_empty, dead_code)]
    pub fn len(&self) -> usize {
        self.images.len()
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_shared::winit::dpi::PhysicalSize;
        use jeriya_test::create_window;

        use crate::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, queue_plan::QueuePlan, surface::Surface,
            swapchain::Swapchain,
        };

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance).unwrap();
            let queue_plan = QueuePlan::new(&instance, &physical_device, iter::once((&window.id(), &surface))).unwrap();
            let device = Device::new(physical_device, &instance, queue_plan).unwrap();
            let swapchain = Swapchain::new(&device, &surface, 2, None).unwrap();
            let size = window.inner_size();
            assert_eq!(swapchain.extent().width, size.width);
            assert_eq!(swapchain.extent().height, size.height);
        }

        #[test]
        fn recreate() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance).unwrap();
            let queue_plan = QueuePlan::new(&instance, &physical_device, iter::once((&window.id(), &surface))).unwrap();
            let device = Device::new(physical_device, &instance, queue_plan).unwrap();
            let mut swapchain = Swapchain::new(&device, &surface, 2, None).unwrap();
            let size = window.inner_size();
            assert_eq!(swapchain.extent().width, size.width);
            assert_eq!(swapchain.extent().height, size.height);

            let new_width = size.width + 2;
            let new_height = size.height + 2;
            let _new_size = window.request_inner_size(PhysicalSize::new(new_width, new_height));
            swapchain = Swapchain::new(&device, &surface, 2, Some(&swapchain)).unwrap();
            assert_eq!(swapchain.extent().width, new_width);
            assert_eq!(swapchain.extent().height, new_height);
        }
    }
}
