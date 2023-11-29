pub mod aabb;
mod debug_info;
mod event_queue;
mod indexing_container;
pub mod obj_writer;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    result,
};

use nalgebra::{Vector3, Vector4};
use serde::{Deserialize, Serialize};
use winit::window::Window;

pub use debug_info::*;
pub use event_queue::*;
pub use indexing_container::*;

pub use async_trait;
pub use bitflags;
pub use bumpalo;
pub use bus;
pub use byte_unit;
pub use byteorder;
pub use chrono;
pub use colors_transform;
pub use crossbeam_channel;
pub use derive_more;
pub use derive_new;
pub use derive_where;
pub use float_cmp;
pub use indoc;
pub use itertools;
pub use kdtree;
pub use log;
pub use maplit;
pub use nalgebra;
pub use nalgebra_glm;
pub use num_cpus;
pub use parking_lot;
pub use pathdiff;
pub use plotters;
pub use rand;
pub use rayon;
pub use spin_sleep;
pub use thiserror;
pub use thread_id;
pub use tracy_client;
pub use walkdir;
pub use winit;

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

/// Color with the components red, green and blue.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ByteColor3 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ByteColor3 {
    /// Creates a new `Color3` with the given red, green and blue components.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Creates a new `Color3` with the given red, green and blue components as floats in the range [0.0, 1.0].
    pub fn from_floats(r: f32, g: f32, b: f32) -> Self {
        Self::new((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
    }

    /// Returns the `Color3` as a `Vector3<f32>`.
    pub fn as_vector3(&self) -> Vector3<f32> {
        Vector3::new(self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0)
    }

    /// Returns the `Color3` as a `Vector4<f32>` with alpha set to 1.0.
    pub fn as_vector4(&self) -> Vector4<f32> {
        Vector4::new(self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, 1.0)
    }

    /// Returns the `Color3` as a `Color4` with alpha set to 1.0.
    pub fn as_byte_color4(&self) -> ByteColor4 {
        ByteColor4::new(self.r, self.g, self.b, 255)
    }
}

impl From<[f32; 3]> for ByteColor3 {
    fn from([r, g, b]: [f32; 3]) -> Self {
        Self::from_floats(r, g, b)
    }
}

/// Color with the components red, green, blue and alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ByteColor4 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ByteColor4 {
    /// Creates a new `Color4` with the given red, green, blue and alpha components.
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a new `Color4` with the given red, green, blue and alpha components as floats in the range [0.0, 1.0].
    pub fn from_floats(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::new((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8)
    }

    /// Returns the `Color4` as a `Vector3<f32>`.
    pub fn as_vector3(&self) -> Vector3<f32> {
        Vector3::new(self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0)
    }

    /// Returns the `Color4` as a `Vector4<f32>`.
    pub fn as_vector4(&self) -> Vector4<f32> {
        Vector4::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        )
    }

    /// Returns the `Color4` as a `Color3` without the alpha component.
    pub fn as_byte_color3(&self) -> ByteColor3 {
        ByteColor3::new(self.r, self.g, self.b)
    }
}

impl From<[f32; 4]> for ByteColor4 {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self::from_floats(r, g, b, a)
    }
}

/// Determines the frame rate at which a window is rendered.
#[derive(Clone, Copy, Debug)]
pub enum FrameRate {
    Unlimited,
    Limited(u32),
}

/// Configuration for the [`Window`]s
#[derive(Clone, Debug)]
pub struct WindowConfig<'w> {
    pub window: &'w Window,
    pub frame_rate: FrameRate,
}

/// Configuration for the [`Renderer`]
pub struct RendererConfig {
    pub application_name: Option<String>,
    pub default_desired_swapchain_length: u32,
    pub maximum_number_of_mesh_attributes: usize,
    pub maximum_number_of_point_cloud_attributes: usize,
    pub maximum_number_of_cameras: usize,
    pub maximum_number_of_camera_instances: usize,
    pub maximum_number_of_rigid_meshes: usize,
    pub maximum_number_of_rigid_mesh_instances: usize,
    pub maximum_number_of_point_clouds: usize,
    pub maximum_number_of_point_cloud_instances: usize,
    pub maximum_meshlets: usize,
    pub maximum_visible_rigid_mesh_instances: usize,
    pub maximum_visible_rigid_mesh_meshlets: usize,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            application_name: None,
            default_desired_swapchain_length: 2,
            maximum_number_of_mesh_attributes: 2usize.pow(10),
            maximum_number_of_point_cloud_attributes: 2usize.pow(10),
            maximum_number_of_cameras: 16,
            maximum_number_of_camera_instances: 64,
            maximum_number_of_rigid_meshes: 2usize.pow(10),
            maximum_number_of_point_clouds: 2usize.pow(10),
            maximum_number_of_point_cloud_instances: 2usize.pow(10),
            maximum_number_of_rigid_mesh_instances: 2usize.pow(10),
            maximum_meshlets: 2usize.pow(20),
            maximum_visible_rigid_mesh_instances: 2usize.pow(10),
            maximum_visible_rigid_mesh_meshlets: 2usize.pow(20),
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
    let r = (hash % (RESOLUTION)) as f32 / RESOLUTION as f32;
    let g = (hash % (RESOLUTION / 3)) as f32 / RESOLUTION as f32;
    let b = (hash % (RESOLUTION / 9)) as f32 / RESOLUTION as f32;

    const BASE: f32 = 0.4;
    let r = BASE + r * (1.0 - BASE);
    let g = BASE + g * (1.0 - BASE);
    let b = BASE + b * (1.0 - BASE);

    Vector4::new(r, g, b, 1.0)
}

/// Returns a random normalized vector
pub fn random_direction() -> Vector3<f32> {
    Vector3::new(rand::random::<f32>(), rand::random::<f32>(), rand::random::<f32>()).normalize()
}
