use crate::swapchain::Swapchain;

/// Dynamic array that has the length of the swapchain
pub struct SwapchainVec<T> {
    data: Vec<T>,
}

impl<T> SwapchainVec<T> {
    /// Creates a new `SwapchainVec<T>` for the given `Swapchain` by using the function `init` to initialize the elements
    pub fn new<F>(swapchain: &Swapchain, init: F) -> crate::Result<Self>
    where
        F: FnMut(usize) -> crate::Result<T>,
    {
        Ok(Self {
            data: (0..swapchain.len()).map(init).collect::<crate::Result<Vec<_>>>()?,
        })
    }

    /// Length of the `SwapchainVec<T>`. This is always the length of the swapchain.
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_test::create_window;

        use crate::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface, swapchain::Swapchain,
            swapchain_vec::SwapchainVec,
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
            let _vec = SwapchainVec::new(&swapchain, |_| Ok(0)).unwrap();
        }
    }
}
