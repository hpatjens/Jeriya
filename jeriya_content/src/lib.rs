//! # Overview
//!
//! Crate for [processing](AssetProcessor) and [importing](AssetImporter) assets.
//!
//! This crate is based around the idea of two structurally mirrored directory
//! trees. One is the source directory, which contains the original assets
//! and the other is the target directory, which contains the processed assets.
//! For every file in the source directory, there is a corresponding file
//! in the target directory which has the same name as the file. The relative
//! location of the file in the source directory is equal to the relative
//! location in the target directory.
//!
//! # Components
//!
//! There are two main components that this crate provides: the [`AssetProcessor`]
//! and the [`AssetImporter`]. The [`AssetProcessor`] is used to process assets
//! from the source directory to the target directory. The [`AssetImporter`] is
//! used to import assets from the target directory into the renderer.

use std::path::PathBuf;

use jeriya_shared::thiserror;

pub mod asset_importer;
pub mod asset_processor;
pub mod common;
pub mod model;
pub mod point_cloud;
pub mod read_asset;
pub mod shader;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),
    #[error("Invalid extension '{0}' in path {1}")]
    InvalidExtension(String, PathBuf),
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Extension not registered: {0}")]
    ExtensionNotRegistered(String),
    #[error("Failed to start directory watcher in directory: {0}")]
    FailedToStartDirectoryWatcher(PathBuf),
    #[error("Failed to start thread pool")]
    FailedToStartThreadPool,
    #[error("Failed to read the asset: {0}")]
    InvalidAssetData(PathBuf),
    #[error("Failed to serialize the asset: {0}")]
    FailedSerialization(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to deserialize the asset: {0}")]
    FailedDeserialization(Box<dyn std::error::Error + Send + Sync>),
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to execute: {0}")]
    FailedToCompileShader(String),
    #[error("Failed to convert from UTF-8: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}
