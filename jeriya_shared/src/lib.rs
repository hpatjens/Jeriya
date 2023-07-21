mod backend;
mod camera;
mod debug_info;
mod event_queue;
pub mod immediate;
mod indexing_container;
mod objects;
mod resources;

use std::result;

pub use backend::*;
pub use camera::*;
pub use debug_info::*;
pub use event_queue::*;
pub use indexing_container::*;
pub use objects::*;
pub use resources::*;

pub use bitflags;
pub use bumpalo;
pub use byte_unit;
pub use byteorder;
pub use chrono;
pub use derive_more;
pub use log;
pub use nalgebra;
pub use nalgebra_glm;
pub use parking_lot;
pub use thiserror;
pub use tracy_client;
pub use winit;

use winit::window::WindowId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssertLevel {
    None,
    Full,
}

pub const ASSERT_LEVEL: AssertLevel = AssertLevel::Full;

/// Assert that can be enabled in debug and release builds
#[macro_export]
macro_rules! assert {
    ($($arg:tt)*) => {
        if $crate::ASSERT_LEVEL == $crate::AssertLevel::Full {
            std::assert!($($arg)*);
        }
    };
}

/// Assert that can be enabled in debug and release builds
#[macro_export]
macro_rules! assert_eq {
    ($($arg:tt)*) => {
        if $crate::ASSERT_LEVEL == $crate::AssertLevel::Full {
            std::assert_eq!($($arg)*);
        }
    };
}

#[derive(Debug)]
pub enum Error {
    ExpectedWindow,
    UnknownWindowId(WindowId),
    MaximumCapacityReached(usize),
    Backend(Box<dyn std::error::Error>),
    InanimateMesh(resources::inanimate_mesh::Error),
}

pub type Result<T> = result::Result<T, Error>;

/// Configuration for the [`Renderer`]
pub struct RendererConfig {
    pub application_name: Option<String>,
    pub default_desired_swapchain_length: u32,
    pub maximum_number_of_cameras: usize,
    pub maximum_number_of_inanimate_meshes: usize,
    pub maximum_number_of_inanimate_mesh_instances: usize,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            application_name: None,
            default_desired_swapchain_length: 2,
            maximum_number_of_cameras: 16,
            maximum_number_of_inanimate_mesh_instances: 2usize.pow(16),
            maximum_number_of_inanimate_meshes: 2usize.pow(10),
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
