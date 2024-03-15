use crate::{frame_index::FrameIndex, swapchain::Swapchain};

/// Dynamic array that has the length of the swapchain
pub struct SwapchainVec<T> {
    data: Vec<T>,
}

impl<T> SwapchainVec<T> {
    const OUT_OF_BOUNDS_MSG: &'static str = "swapchain index was out of bounds while accessing a SwapchainVec";
    const MUST_BE_SET_MST: &'static str = "swapchain index must be set before accessing a SwapchainVec";

    /// Creates a new `SwapchainVec<T>` for the given `Swapchain` by using the function `init` to initialize the elements
    pub fn new<F>(swapchain: &Swapchain, init: F) -> crate::Result<Self>
    where
        F: FnMut(usize) -> crate::Result<T>,
    {
        Ok(Self {
            data: (0..swapchain.len()).map(init).collect::<crate::Result<Vec<_>>>()?,
        })
    }

    /// Returns an iterator over the data
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    /// Returns a mutable iterator over the data
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    /// Length of the `SwapchainVec<T>`. This is always the length of the swapchain.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns a reference to the entry in the `SwapchainVec` that belongs to the given `FrameIndex`.
    pub fn get(&self, frame_index: &FrameIndex) -> &T {
        let index = frame_index.swapchain_index().expect(Self::MUST_BE_SET_MST);
        self.data.get(index).expect(Self::OUT_OF_BOUNDS_MSG)
    }

    /// Returns a mutable reference to the entry in the `SwapchainVec` that belongs to the given `FrameIndex`.
    pub fn get_mut(&mut self, frame_index: &FrameIndex) -> &mut T {
        let index = frame_index.swapchain_index().expect(Self::MUST_BE_SET_MST);
        self.data.get_mut(index).expect(Self::OUT_OF_BOUNDS_MSG)
    }
}

impl<T> SwapchainVec<Option<T>> {
    /// Replaces the value at `frame_index` with `value`
    pub fn replace(&mut self, frame_index: &FrameIndex, value: T) -> Option<T> {
        let index = frame_index.swapchain_index().expect(Self::MUST_BE_SET_MST);
        self.data.get_mut(index).expect(Self::OUT_OF_BOUNDS_MSG).replace(value)
    }
}

impl<T> IntoIterator for SwapchainVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'s, T> IntoIterator for &'s SwapchainVec<T> {
    type Item = &'s T;
    type IntoIter = std::slice::Iter<'s, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl<'s, T> IntoIterator for &'s mut SwapchainVec<T> {
    type Item = &'s mut T;
    type IntoIter = std::slice::IterMut<'s, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_test::create_window;

        use crate::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, queue_plan::QueuePlan, surface::Surface,
            swapchain::Swapchain, swapchain_vec::SwapchainVec,
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
            let _vec = SwapchainVec::new(&swapchain, |_| Ok(0)).unwrap();
        }

        #[test]
        fn iter() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance).unwrap();
            let queue_plan = QueuePlan::new(&instance, &physical_device, iter::once((&window.id(), &surface))).unwrap();
            let device = Device::new(physical_device, &instance, queue_plan).unwrap();
            let swapchain = Swapchain::new(&device, &surface, 2, None).unwrap();
            let mut vec = SwapchainVec::new(&swapchain, |_| Ok(0)).unwrap();
            for _ in &vec {}
            for _ in &mut vec {}
            for _ in vec {}
        }
    }
}
