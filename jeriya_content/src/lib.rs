//! # Overview
//!
//! Crate for [processing](AssetProcessor) and [importing](AssetImporter) assets.
//!
//! This crate is based around the idea of two structurally mirrored directory
//! trees. One is the source directory, which contains the original assets
//! and the other is the target directory, which contains the processed assets.
//! For every file in the source directory, there is a corresponding *directory*
//! in the target directory which has the same name as the file. The relative
//! location of the file in the source directory is the relative location of
//! the directory in the target directory. Inside the directory in the target
//! directory, there are the files containing the processed assets as well as
//! a `asset.yaml` file which contains metadata.
//!
//! ## Example:
//!
//! **Source Directory:**
//!
//! ```text
//! assets/
//! ├─ textures/
//! │  ├─ texture1.png
//! │  ├─ texture2.png
//! ├─ models/
//! │  ├─ model1.fbx
//! ```
//!
//! **Target Directory:**
//!
//! ```text
//! assets/
//! ├─ textures/
//! │  ├─ texture1.png
//! │  │  ├─ texture1.bin
//! │  │  ├─ asset.yaml
//! │  ├─ texture2.png
//! │  │  ├─ texture2.bin
//! │  │  ├─ asset.yaml
//! ├─ models/
//! │  ├─ model1.fbx
//! │  │  ├─ model1.bin
//! │  │  ├─ asset.yaml
//! ```
//!
//! # Components
//!
//! There are two main components that this crate provides: the [`AssetProcessor`]
//! and the [`AssetImporter`]. The [`AssetProcessor`] is used to process assets
//! from the source directory to the target directory. The [`AssetImporter`] is
//! used to import assets from the target directory into the renderer.

mod asset_importer;
mod asset_processor;
mod common;

pub mod model;
pub mod point_cloud {
    pub struct PointCloud {}
}

pub use asset_importer::*;
pub use asset_processor::*;
pub use common::{AssetKey, Directories, Error, Result};
