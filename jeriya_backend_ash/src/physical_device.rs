use ash::vk::{self, PhysicalDeviceType};
use jeriya_shared::log::info;

use crate::{instance::Instance, surface::Surface, AsRawVulkan};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find physical devices")]
    NoPhysicalDevices,
    #[error("Failed to find suitable queues")]
    NoSuitableQueues,
}

#[derive(Debug)]
pub struct SuitableQueueFamilyInfo {
    pub queue_family_index: u32,
    pub queue_count: u32,
    _private: (),
}

#[derive(Debug)]
pub struct PhysicalDevice {
    pub suitable_presentation_graphics_queue_family_infos: Vec<SuitableQueueFamilyInfo>,
    pub physical_device_properties: vk::PhysicalDeviceProperties,
    physical_device: vk::PhysicalDevice,
}

impl AsRawVulkan for PhysicalDevice {
    type Output = vk::PhysicalDevice;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.physical_device
    }
}

impl PhysicalDevice {
    /// Select a physical device that can be used for the device creation
    pub fn new(instance: &Instance, surfaces: &[Surface]) -> crate::Result<PhysicalDevice> {
        let instance = instance.as_raw_vulkan();

        // Get Physical Devices
        let physical_devices = unsafe { instance.enumerate_physical_devices()? };
        if physical_devices.is_empty() {
            return Err(crate::Error::PhysicalDeviceError(Error::NoPhysicalDevices));
        }

        // Rate Physical Devices
        let rated = rate_physical_devices(instance, physical_devices)?;

        // Select the Physical Device which has the required queues
        for physical_device in rated {
            let physical_device_properties = unsafe { instance.get_physical_device_properties(physical_device) };
            info!("Available Physical Device: {:#?}", physical_device_properties);

            let queues = get_presentation_graphics_queue_families(instance, &physical_device, surfaces)?;
            if !queues.is_empty() {
                return Ok(PhysicalDevice {
                    suitable_presentation_graphics_queue_family_infos: queues,
                    physical_device_properties,
                    physical_device,
                });
            }
        }

        Err(crate::Error::PhysicalDeviceError(Error::NoSuitableQueues))
    }
}

/// Rate the physical devices based on some characteristics so that the most capable is selected
fn rate_physical_devices(instance: &ash::Instance, physical_devices: Vec<vk::PhysicalDevice>) -> crate::Result<Vec<vk::PhysicalDevice>> {
    let mut rated = physical_devices
        .into_iter()
        .map(|physical_device| {
            let physical_device_properties = unsafe { instance.get_physical_device_properties(physical_device) };
            let name = jeriya_shared::c_null_terminated_char_array_to_string(&physical_device_properties.device_name)?;
            let mut rating = 0;
            if physical_device_properties.device_type == PhysicalDeviceType::DISCRETE_GPU {
                rating += 1;
            }
            Ok((rating, name, physical_device))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let list = rated
        .iter()
        .map(|device| format!("({}, {})", device.0, device.1))
        .collect::<Vec<String>>()
        .join(", ");
    info!("Rated Physical Devices: {list}");
    rated.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rated
        .into_iter()
        .map(|(_, _, physical_device)| physical_device)
        .collect::<Vec<vk::PhysicalDevice>>())
}

/// Returns the queue families that can be used for presentation
fn get_presentation_graphics_queue_families(
    instance: &ash::Instance,
    physical_device: &vk::PhysicalDevice,
    surfaces: &[Surface],
) -> crate::Result<Vec<SuitableQueueFamilyInfo>> {
    let physical_device_queue_family_properties = unsafe { instance.get_physical_device_queue_family_properties(*physical_device) };
    let mut queues = Vec::new();
    for (queue_family_index, queue_family_properties) in physical_device_queue_family_properties.iter().enumerate() {
        let mut supported = true;
        for surface in surfaces {
            if !surface.supports_presentation(physical_device, queue_family_index)? {
                supported = false;
            }
        }
        if supported {
            queues.push(SuitableQueueFamilyInfo {
                queue_family_index: queue_family_index as u32,
                queue_count: queue_family_properties.queue_count,
                _private: (),
            });
        }
    }
    Ok(queues)
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_test::create_window;

        use crate::{entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface};

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, &"my_application", true).unwrap();
            let surfaces = [Surface::new(&entry, &instance, &window).unwrap()];
            let _physical_device = PhysicalDevice::new(&instance, &surfaces).unwrap();
        }
    }
}
