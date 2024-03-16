use std::{
    ffi::{CStr, CString},
    sync::Arc,
};

use crate::{entry::Entry, AsRawVulkan, Error, IntoJeriya, Result};

use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{self},
    },
    vk::{self},
};
use jeriya_shared::log::info;

/// Wrapper for [`ash::Instance`]
pub struct Instance {
    pub available_layers: Vec<String>,
    pub active_layers: Vec<String>,
    pub active_extensions: Vec<String>,
    pub debug_utils: Option<DebugUtils>,
    ash_instance: ash::Instance,
    _entry: Arc<Entry>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.ash_instance.destroy_instance(None) };
    }
}

impl Instance {
    /// Creates a Vulkan instance with a default configuration of layers and extensions
    pub fn new(entry: &Arc<Entry>, application_name: &str, enable_validation_layer: bool) -> Result<Arc<Instance>> {
        let application_name = CString::new(application_name).unwrap();

        // Available Layers
        let available_layers = available_layers(entry)?;
        info!("Available Layers:\n{}", list_strings(&available_layers));

        // Active Layers
        let mut active_layers = Vec::new();
        if enable_validation_layer {
            active_layers.extend(
                available_layers
                    .iter()
                    .filter(|&layer| layer == "VK_LAYER_LUNARG_standard_validation" || layer == "VK_LAYER_KHRONOS_validation")
                    .cloned()
                    .collect::<Vec<_>>(),
            );
        }
        info!("Active Layers:\n{}", list_strings(&active_layers));

        // Active Extensions
        fn expect_extension(extension_name: &'static CStr) -> String {
            extension_name.to_str().expect("failed to convert extension name").to_owned()
        }
        let mut active_extensions = vec![
            expect_extension(khr::Surface::name()),
            expect_extension(khr::Win32Surface::name()),
            expect_extension(khr::GetPhysicalDeviceProperties2::name()),
        ];
        if enable_validation_layer {
            active_extensions.push(expect_extension(DebugUtils::name()));
        }
        info!("Active Extensions:\n{}", list_strings(&active_extensions));

        let active_layers_c = active_layers
            .iter()
            .cloned()
            .map(|layer| CString::new(layer.as_str()).map_err(Error::StringNulError))
            .collect::<Result<Vec<_>>>()?;
        let active_extension_c = active_extensions
            .iter()
            .map(|s| CString::new(s.clone()).map_err(Error::StringNulError))
            .collect::<Result<Vec<_>>>()?;

        let layers_names_ptrs: Vec<*const i8> = active_layers_c.iter().map(|raw_name| raw_name.as_ptr()).collect();
        let extension_names_ptrs: Vec<*const i8> = active_extension_c.iter().map(|raw_name| raw_name.as_ptr()).collect();

        let appinfo = vk::ApplicationInfo::builder()
            .application_name(&application_name)
            .application_version(0)
            .engine_name(&application_name)
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 3, 0));

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&appinfo)
            .enabled_layer_names(&layers_names_ptrs)
            .enabled_extension_names(&extension_names_ptrs);

        let ash_instance = unsafe { entry.as_raw_vulkan().create_instance(&create_info, None).into_jeriya()? };

        let debug_utils = if enable_validation_layer {
            Some(DebugUtils::new(entry.as_raw_vulkan(), &ash_instance))
        } else {
            None
        };

        Ok(Arc::new(Instance {
            _entry: entry.clone(),
            ash_instance,
            available_layers,
            active_layers,
            active_extensions,
            debug_utils,
        }))
    }
}

impl AsRawVulkan for Instance {
    type Output = ash::Instance;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.ash_instance
    }
}

fn list_strings(strings: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    strings
        .into_iter()
        .map(|s| format!("\t{}", s.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn available_layers(entry: &Entry) -> Result<Vec<String>> {
    let layer_properties = entry.as_raw_vulkan().enumerate_instance_layer_properties()?;
    let result = layer_properties
        .iter()
        .map(|properties| &properties.layer_name)
        .map(|array| jeriya_shared::c_null_terminated_char_array_to_string(array).map_err(Error::StringUtf8Error))
        .collect::<Result<Vec<_>>>()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_instance_validation_active() {
        let entry = Entry::new().unwrap();
        let _instance = Instance::new(&entry, "my_test_application", true).unwrap();
    }

    #[test]
    fn create_instance_validation_inactive() {
        let entry = Entry::new().unwrap();
        let _instance = Instance::new(&entry, "my_test_application", false).unwrap();
    }
}
