use std::sync::Arc;

use ash::{extensions::khr, vk};

use crate::{instance::Instance, physical_device::PhysicalDevice, queue::Queue, AsRawVulkan};

pub struct Device {
    pub presentation_queue: Queue,
    device: ash::Device,
    pub physical_device: PhysicalDevice,
    _instance: Arc<Instance>,
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { self.device.destroy_device(None) };
    }
}

impl Device {
    /// Creates a new `Device` based on the given [`PhysicalDevice`].
    pub fn new(physical_device: PhysicalDevice, instance: &Arc<Instance>) -> crate::Result<Arc<Self>> {
        let features = vk::PhysicalDeviceFeatures::default();
        let queue_priorities = physical_device
            .suitable_presentation_graphics_queue_family_infos
            .iter()
            .map(|suitable_queue_family_info| {
                std::iter::repeat(1.0)
                    .take(suitable_queue_family_info.queue_count as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let queue_infos = physical_device
            .suitable_presentation_graphics_queue_family_infos
            .iter()
            .zip(queue_priorities.iter())
            .map(|(suitable_queue_family_info, priorities)| {
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(suitable_queue_family_info.queue_family_index)
                    .queue_priorities(priorities)
                    .build()
            })
            .collect::<Vec<_>>();
        let device_extension_names_raw = [khr::Swapchain::name().as_ptr()];
        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extension_names_raw)
            .enabled_features(&features);
        let device = unsafe {
            instance
                .as_raw_vulkan()
                .create_device(*physical_device.as_raw_vulkan(), &device_create_info, None)?
        };

        // Queues
        assert!(!physical_device.suitable_presentation_graphics_queue_family_infos.is_empty());
        assert!(physical_device.suitable_presentation_graphics_queue_family_infos[0].queue_count > 0);
        let queue_family_index = physical_device.suitable_presentation_graphics_queue_family_infos[0].queue_family_index;
        let queue_index = 0;
        let presentation_queue = unsafe { Queue::get_from_family(&device, queue_family_index, queue_index) };

        Ok(Arc::new(Device {
            presentation_queue,
            device,
            physical_device,
            _instance: instance.clone(),
        }))
    }
}

impl AsRawVulkan for Device {
    type Output = ash::Device;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_test::create_window;

        use crate::{device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface};

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance, &[surface]).unwrap();
            let _device = Device::new(physical_device, &instance).unwrap();
        }
    }
}
