mod instance;

use std::{ffi::NulError, str::Utf8Error};

use jeriya::Backend;

use ash::{
    prelude::VkResult,
    vk::{self},
    Entry, Instance, LoadingError,
};
use jeriya_shared::{log::info, winit::window::Window};

use crate::instance::create_instance;

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) trait IntoJeriya {
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
    #[error("Error while converting a string to UTF-8: {:?}", .0)]
    StringUtf8Error(#[from] Utf8Error),
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
