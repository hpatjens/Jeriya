use std::sync::Arc;

use ash::vk;

use crate::{device::Device, AsRawVulkan};

pub struct Semaphore {
    semaphore: vk::Semaphore,
    device: Arc<Device>,
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe { self.device.as_raw_vulkan().destroy_semaphore(self.semaphore, None) }
    }
}

impl Semaphore {
    pub fn new(device: &Arc<Device>) -> crate::Result<Self> {
        Ok(Self {
            semaphore: unsafe { device.as_raw_vulkan().create_semaphore(&Default::default(), None)? },
            device: device.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_test::create_window;

        use crate::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, semaphore::Semaphore, surface::Surface,
        };

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let _semaphore = Semaphore::new(&device).unwrap();
        }
    }
}
