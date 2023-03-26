use std::ffi::{CStr, CString};

use crate::{Error, IntoJeriya, Result};

use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{self},
    },
    vk::{self},
    Entry, Instance,
};
use jeriya_shared::log::info;

fn list_strings<'a>(strings: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    strings
        .into_iter()
        .map(|s| format!("\t{}", s.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn available_layers(entry: &Entry) -> Result<Vec<String>> {
    let layer_properties = entry.enumerate_instance_layer_properties()?;
    let result = layer_properties
        .iter()
        .map(|properties| &properties.layer_name)
        .map(|array| Ok(jeriya_shared::c_null_terminated_char_array_to_string(array).map_err(|err| Error::StringUtf8Error(err))?))
        .collect::<Result<Vec<_>>>()?;
    Ok(result)
}

/// Creates a Vulkan instance with a default configuration of layers and extensions
pub fn create_instance(entry: &Entry, application_name: &str, enable_validation_layer: bool) -> Result<Instance> {
    let application_name = CString::new(application_name).unwrap();

    // Available Layers
    let available_layers = available_layers(entry)?;
    info!("Available Layers:\n{}", list_strings(&available_layers));

    // Active Layers
    let mut layer_names = Vec::new();
    if enable_validation_layer {
        layer_names.extend(
            available_layers
                .into_iter()
                .filter(|layer| layer == &"VK_LAYER_LUNARG_standard_validation" || layer == &"VK_LAYER_KHRONOS_validation")
                .collect::<Vec<_>>(),
        );
    }
    info!("Active Layers:\n{}", list_strings(&layer_names));

    // Active Extensions
    fn expect_extension(extension_name: &'static CStr) -> &str {
        extension_name.to_str().expect("failed to converte extension name")
    }
    let mut extension_names = vec![expect_extension(khr::Surface::name()), expect_extension(khr::Win32Surface::name())];
    if enable_validation_layer {
        extension_names.push(expect_extension(DebugUtils::name()));
    }
    info!("Active Extensions:\n{}", list_strings(&extension_names));

    let layer_names = layer_names
        .into_iter()
        .map(|layer| CString::new(layer.as_str()).map_err(|err| Error::StringNulError(err)))
        .collect::<Result<Vec<_>>>()?;
    let extension_names = extension_names
        .iter()
        .map(|s| CString::new(*s).map_err(|err| Error::StringNulError(err)))
        .collect::<Result<Vec<_>>>()?;

    let layers_names_ptrs: Vec<*const i8> = layer_names.iter().map(|raw_name| raw_name.as_ptr()).collect();
    let extension_names_ptrs: Vec<*const i8> = extension_names.iter().map(|raw_name| raw_name.as_ptr()).collect();

    let appinfo = vk::ApplicationInfo::builder()
        .application_name(&application_name)
        .application_version(0)
        .engine_name(&application_name)
        .engine_version(0)
        .api_version(vk::make_api_version(0, 1, 0, 0));

    let create_info = vk::InstanceCreateInfo::builder()
        .application_info(&appinfo)
        .enabled_layer_names(&layers_names_ptrs)
        .enabled_extension_names(&extension_names_ptrs);

    unsafe { entry.create_instance(&create_info, None).into_jeriya() }
}
