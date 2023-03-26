use std::ffi::{CStr, CString, NulError};

use jeriya::Backend;

use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{self},
    },
    prelude::VkResult,
    vk::{self},
    Entry, Instance, LoadingError,
};
use jeriya_shared::{log::info, winit::window::Window};

pub type Result<T> = std::result::Result<T, Error>;

trait IntoJeriya {
    type Output;
    fn into_jeriya(self) -> Self::Output;
}

impl<T> IntoJeriya for VkResult<T> {
    type Output = Result<T>;

    fn into_jeriya(self) -> Self::Output {
        self.map_err(|err| Error::Result(err))
    }
}

/// Errors in the ash backend
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error while loading Vulkan {:?}", .0)]
    LoadingError(#[from] LoadingError),
    #[error("Error while executing a Vulkan operation {:?}", .0)]
    Result(#[from] vk::Result),
    #[error("Error while converting a string: {:?}", .0)]
    StringNulError(#[from] NulError),
}

impl From<Error> for jeriya_shared::Error {
    fn from(value: Error) -> Self {
        jeriya_shared::Error::Backend(Box::new(value))
    }
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
            return Err(jeriya_shared::Error::ExpectedWindow);
        }

        info!("Creating Vulkan Entry");
        let entry = unsafe { Entry::load().map_err(Error::LoadingError)? };

        info!("Creating Vulkan Instance");
        let application_name = application_name.unwrap_or(env!("CARGO_PKG_NAME"));
        let instance = create_instance(&entry, application_name)?;

        Ok(Self { instance })
    }
}

pub fn list_strings<'a>(strings: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    strings
        .into_iter()
        .map(|s| format!("\t{}", s.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn available_layers(entry: &Entry) -> Vec<String> {
    entry
        .enumerate_instance_layer_properties()
        .unwrap()
        .iter()
        .map(|properties| &properties.layer_name)
        .map(|array| jeriya_shared::c_null_terminated_char_array_to_string(array).unwrap())
        .collect::<Vec<_>>()
}

pub fn create_instance(entry: &Entry, application_name: &str) -> Result<Instance> {
    let application_name = CString::new(application_name).unwrap();

    // Available Layers
    let available_layers = available_layers(entry);
    info!("Available Layers:\n{}", list_strings(&available_layers));

    // Active Layers
    let layer_names = available_layers
        .into_iter()
        .filter(|layer| layer == &"VK_LAYER_LUNARG_standard_validation" || layer == &"VK_LAYER_KHRONOS_validation")
        .collect::<Vec<_>>();
    info!("Active Layers:\n{}", list_strings(&layer_names));

    // Active Extensions
    fn expect_extension(extension_name: &'static CStr) -> &str {
        extension_name.to_str().expect("failed to converte extension name")
    }
    let extension_names = vec![
        expect_extension(khr::Surface::name()),
        expect_extension(khr::Win32Surface::name()),
        expect_extension(DebugUtils::name()),
    ];
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

#[cfg(test)]
#[cfg(not(feature = "ignore_in_ci"))]
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
            assert!(matches!(Ash::new(None, &[]), Err(jeriya_shared::Error::ExpectedWindow)));
        }
    }
}
