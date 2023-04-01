use std::{default::Default, os::raw::c_void, ptr, sync::Arc};

use ash::{
    extensions::khr::{self},
    vk,
};

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winapi::um::libloaderapi::GetModuleHandleW;

use crate::{entry::Entry, instance::Instance, AsRawVulkan, Error};
use jeriya_shared::winit;

/// Surface of a window to create the `Swapchain`.
pub struct Surface {
    pub(crate) surface_khr: vk::SurfaceKHR,
    pub(crate) surface: khr::Surface,
    _entry: Arc<Entry>,
}

impl Surface {
    /// Creates a new `Surface` for the given window.
    pub fn new(entry: &Arc<Entry>, instance: &Arc<Instance>, window: &winit::window::Window) -> crate::Result<Surface> {
        let surface_khr = unsafe { create_surface_khr(entry, instance, window) }?;
        let surface = khr::Surface::new(entry.as_raw_vulkan(), &instance.as_raw_vulkan());
        Ok(Surface {
            surface_khr,
            surface,
            _entry: entry.clone(),
        })
    }

    /// Returns whether the given queue family index of the physical device supports presentation
    pub fn supports_presentation(&self, physical_device: &vk::PhysicalDevice, queue_family_index: usize) -> crate::Result<bool> {
        unsafe {
            Ok(self
                .surface
                .get_physical_device_surface_support(*physical_device, queue_family_index as u32, self.surface_khr)?)
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.surface.destroy_surface(self.surface_khr, None) };
    }
}

#[cfg(target_os = "windows")]
unsafe fn create_surface_khr(entry: &Entry, instance: &Instance, window: &winit::window::Window) -> crate::Result<vk::SurfaceKHR> {
    let hwnd = if let RawWindowHandle::Win32(windows_handle) = window.raw_window_handle() {
        windows_handle.hwnd
    } else {
        return Err(Error::WrongPlatform);
    };
    let hinstance = GetModuleHandleW(ptr::null()) as *const c_void;
    let win32_create_info = vk::Win32SurfaceCreateInfoKHR {
        hinstance,
        hwnd,
        ..Default::default()
    };
    let win32_surface_loader = khr::Win32Surface::new(entry.as_raw_vulkan(), instance.as_raw_vulkan());
    Ok(win32_surface_loader.create_win32_surface(&win32_create_info, None)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod backend_new {
        use jeriya_test::create_window;

        use super::*;

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, &"my_application", true).unwrap();
            let _surface = Surface::new(&entry, &instance, &window).unwrap();
        }
    }
}
