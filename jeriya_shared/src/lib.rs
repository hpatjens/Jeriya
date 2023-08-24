mod backend;
mod camera;
mod debug_info;
mod event_queue;
pub mod immediate;
mod indexing_container;
mod objects;
mod resources;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    result,
};

pub use backend::*;
pub use camera::*;
pub use debug_info::*;
pub use event_queue::*;
pub use indexing_container::*;
use nalgebra::Vector4;
pub use objects::*;
pub use resources::*;

pub use async_trait;
pub use bitflags;
pub use bumpalo;
pub use bus;
pub use byte_unit;
pub use byteorder;
pub use chrono;
pub use crossbeam_channel;
pub use derive_more;
pub use indoc;
pub use log;
pub use nalgebra;
pub use nalgebra_glm;
pub use parking_lot;
pub use pathdiff;
pub use rand;
pub use rayon;
pub use thiserror;
pub use tracy_client;
pub use walkdir;
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

/// Error type for the whole library
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No windows are given")]
    ExpectedWindow,
    #[error("The given window id is not known")]
    UnknownWindowId(WindowId),
    #[error("The maximum capacity of elements is reached: {0}")]
    MaximumCapacityReached(usize),
    #[error("Error from the backend: {0}")]
    Backend(Box<dyn std::error::Error + Send + Sync>),
    #[error("Error concerning the InanimateMeshes")]
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

pub fn leak_string(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[macro_export]
macro_rules! plot_with_index {
    ($prefix:literal, $index:expr, $value:expr) => {{
        match $index {
            0 => plot!(concat!($prefix, "0"), $value),
            1 => plot!(concat!($prefix, "1"), $value),
            2 => plot!(concat!($prefix, "2"), $value),
            3 => plot!(concat!($prefix, "3"), $value),
            4 => plot!(concat!($prefix, "4"), $value),
            5 => plot!(concat!($prefix, "5"), $value),
            6 => plot!(concat!($prefix, "6"), $value),
            _ => plot!(concat!($prefix, "unknown"), $value),
        }
    }};
}

/// Returns a random color with alpha set to 1.0
pub fn pseudo_random_color(index: usize) -> Vector4<f32> {
    fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    let hash = calculate_hash(&index);

    const RESOLUTION: u64 = 36;
    let r = (hash % (RESOLUTION / 1)) as f32 / RESOLUTION as f32;
    let g = (hash % (RESOLUTION / 3)) as f32 / RESOLUTION as f32;
    let b = (hash % (RESOLUTION / 9)) as f32 / RESOLUTION as f32;

    const BASE: f32 = 0.4;
    let r = BASE + r * (1.0 - BASE);
    let g = BASE + g * (1.0 - BASE);
    let b = BASE + b * (1.0 - BASE);

    Vector4::new(r, g, b, 1.0)
}
