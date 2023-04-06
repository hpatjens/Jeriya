mod debug_info;

use std::result;

pub use debug_info::*;

pub use chrono;
pub use log;
pub use parking_lot;
pub use winit;

#[derive(Debug)]
pub enum Error {
    ExpectedWindow,
    Backend(Box<dyn std::error::Error>),
}

pub type Result<T> = result::Result<T, Error>;

/// Configuration for the [`Renderer`]
pub struct RendererConfig {
    pub application_name: Option<String>,
    pub default_desired_swapchain_length: u32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            application_name: None,
            default_desired_swapchain_length: 2,
        }
    }
}

/// Name of the function this macro is called in
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[..name.len() - 3]
    }};
}

/// Creates a String from the given char array. It's expected that `char_array` contains a 0.
pub fn c_null_terminated_char_array_to_string(char_array: &[i8]) -> result::Result<String, std::str::Utf8Error> {
    assert!(char_array.iter().any(|c| *c == 0), "\"char_array\" is not null terminated.");
    let chars = char_array.iter().take_while(|c| **c != 0).map(|i| *i as u8).collect::<Vec<_>>();
    Ok(std::str::from_utf8(chars.as_slice())?.to_owned())
}
