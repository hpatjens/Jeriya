use std::ffi::CString;

use jeriya::Backend;

use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{self},
    },
    vk::{self, Result},
    Entry, Instance, LoadingError,
};
use jeriya_shared::{log::info, winit::window::Window, Error};
use thiserror::Error;

/// Errors in the ash backend
#[derive(Error, Debug)]
pub enum AshError {
    #[error("Error while loading Vulkan {:?}", .0)]
    LoadingError(LoadingError),
    #[error("Error while executing a Vulkan operation {:?}", .0)]
    Result(Result),
}

pub struct Ash {
    instance: Instance,
}

impl Backend for Ash {
    fn new(application_name: Option<&str>, windows: &[&Window]) -> jeriya_shared::Result<Self>
    where
        Self: Sized,
    {
        if windows.is_empty() {
            return Err(Error::ExpectedWindow);
        }

        let entry = unsafe { Entry::load().map_err(|err| Error::Backend(Box::new(err)))? };
        let application_name = application_name.unwrap_or(env!("CARGO_PKG_NAME"));
        let instance = create_instance(&entry, application_name);

        Ok(Self { instance })
    }
}

pub fn create_instance(entry: &Entry, application_name: &str) -> Instance {
    let application_name = CString::new(application_name).unwrap();

    let available_layers = entry
        .enumerate_instance_layer_properties()
        .unwrap()
        .iter()
        .map(|properties| &properties.layer_name)
        .map(|array| jeriya_shared::c_null_terminated_char_array_to_string(array).unwrap())
        .collect::<Vec<_>>();

    let available_layers_string = available_layers.iter().map(|s| format!("\t{}", s)).collect::<Vec<_>>().join("\n");
    info!("Available Layers:\n{}", available_layers_string);

    let layer_names = available_layers
        .iter()
        .filter(|layer| layer == &"VK_LAYER_LUNARG_standard_validation" || layer == &"VK_LAYER_KHRONOS_validation")
        .map(|layer| CString::new(layer.as_str()).unwrap())
        .collect::<Vec<_>>();

    let layers_names_raw: Vec<*const i8> = layer_names.iter().map(|raw_name| raw_name.as_ptr()).collect();

    let extension_names_raw = vec![
        khr::Surface::name().as_ptr(),
        khr::Win32Surface::name().as_ptr(),
        DebugUtils::name().as_ptr(),
    ];

    let appinfo = vk::ApplicationInfo::builder()
        .application_name(&application_name)
        .application_version(0)
        .engine_name(&application_name)
        .engine_version(0)
        .api_version(vk::make_api_version(0, 1, 0, 0));

    let create_info = vk::InstanceCreateInfo::builder()
        .application_info(&appinfo)
        .enabled_layer_names(&layers_names_raw)
        .enabled_extension_names(&extension_names_raw);

    unsafe { entry.create_instance(&create_info, None).expect("Instance creation error") }
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
            Ash::new(Some("my_application"), &[&window]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            Ash::new(None, &[&window]).unwrap();
        }

        #[test]
        fn empty_windows_none() {
            assert!(matches!(Ash::new(None, &[]), Err(Error::ExpectedWindow)));
        }
    }
}
