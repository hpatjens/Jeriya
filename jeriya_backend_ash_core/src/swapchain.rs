use ash::{extensions::khr, prelude::VkResult, vk};
use jeriya_shared::log::{info, warn};

use std::{cell::RefCell, ops::Drop, sync::Arc};

use crate::{device::Device, instance::Instance, queue::Queue, surface::Surface, AsRawVulkan, Error};

/// Represents the swapchain. This value only changes internally when the swapchain has to be recreated.
pub struct Swapchain {
    inner: RefCell<Inner>,
    surface: Arc<Surface>,
    device: Arc<Device>,
    instance: Arc<Instance>,
}

impl Swapchain {
    /// Creates a new swapchain for the given [`Surface`].
    pub fn new(instance: &Arc<Instance>, device: &Arc<Device>, surface: &Arc<Surface>) -> crate::Result<Self> {
        let inner = Inner::create_swapchain(instance, device, surface, None)?;
        Ok(Self {
            inner: RefCell::new(inner),
            instance: instance.clone(),
            device: device.clone(),
            surface: surface.clone(),
        })
    }

    /// Recreate the swapchain when the resolution changes.
    pub fn recreate(&self) -> crate::Result<()> {
        let mut inner = self.inner.borrow_mut();
        *inner = Inner::create_swapchain(&self.instance, &self.device, &self.surface, Some(&inner.swapchain_khr))?;
        Ok(())
    }

    pub fn acquire_next_image(&self, semaphore_to_signal: vk::Semaphore) -> VkResult<u32> {
        unsafe {
            let inner = self.inner.borrow();
            let (present_index, is_suboptimal) =
                inner
                    .swapchain
                    .acquire_next_image(inner.swapchain_khr, std::u64::MAX, semaphore_to_signal, vk::Fence::null())?;
            if is_suboptimal {
                warn!("Suboptimal swapchain image");
            }
            Ok(present_index)
        }
    }

    pub fn present(
        &self,
        swapchain_image_index: u32,
        rendering_complete_semaphore: vk::Semaphore,
        present_queue: &Queue,
    ) -> crate::Result<()> {
        let inner = self.inner.borrow();
        let wait_semaphors = [rendering_complete_semaphore];
        let swapchains = [inner.swapchain_khr];
        let image_indices = [swapchain_image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphors)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe {
            inner.swapchain.queue_present(*present_queue.as_raw_vulkan(), &present_info)?;
        }
        Ok(())
    }

    /// Returns a copy of the `ImageView`s for the swapchain images
    pub fn image_views(&self) -> Vec<vk::ImageView> {
        self.inner.borrow().image_views.clone()
    }

    #[allow(dead_code)]
    pub fn extent(&self) -> vk::Extent2D {
        self.inner.borrow().extent
    }

    #[allow(dead_code)]
    pub fn surface_format(&self) -> vk::SurfaceFormatKHR {
        self.inner.borrow()._format
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.inner.borrow()._images.len()
    }
}

struct Inner {
    swapchain: khr::Swapchain,
    swapchain_khr: vk::SwapchainKHR,
    _images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    _format: vk::SurfaceFormatKHR,
    extent: vk::Extent2D,
    device: Arc<Device>,
}

impl Drop for Inner {
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

impl Inner {
    fn create_swapchain(
        instance: &Arc<Instance>,
        device: &Arc<Device>,
        surface: &Surface,
        previous_swapchain: Option<&vk::SwapchainKHR>,
    ) -> crate::Result<Self> {
        let surface_capabilities = unsafe {
            surface
                .surface
                .get_physical_device_surface_capabilities(*device.physical_device.as_raw_vulkan(), surface.surface_khr)?
        };
        info!("Surface capabilities: {surface_capabilities:?}");

        // Image Count
        let desired_image_count = {
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0 && desired_image_count > surface_capabilities.max_image_count {
                desired_image_count = surface_capabilities.max_image_count;
            }
            info!("Desired image count: {}", desired_image_count);
            desired_image_count
        };

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
        let swapchain_loader = khr::Swapchain::new(instance.as_raw_vulkan(), device.as_raw_vulkan());
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
                swapchain_create_info = swapchain_create_info.old_swapchain(*previous_swapchain);
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
            _images: images,
            image_views,
            _format: format,
            extent,
            device: device.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_shared::winit::dpi::PhysicalSize;
        use jeriya_test::create_window;

        use crate::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface, swapchain::Swapchain,
        };

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let swapchain = Swapchain::new(&instance, &device, &surface).unwrap();
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
            let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let swapchain = Swapchain::new(&instance, &device, &surface).unwrap();
            let size = window.inner_size();
            assert_eq!(swapchain.extent().width, size.width);
            assert_eq!(swapchain.extent().height, size.height);

            let new_width = size.width + 2;
            let new_height = size.height + 2;
            window.set_inner_size(PhysicalSize::new(new_width, new_height));
            swapchain.recreate().unwrap();
            assert_eq!(swapchain.extent().width, new_width);
            assert_eq!(swapchain.extent().height, new_height);
        }
    }
}
