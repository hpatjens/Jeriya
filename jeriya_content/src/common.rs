use std::{
    io,
    path::{Path, PathBuf},
    result,
    time::SystemTime,
};

use jeriya_shared::thiserror;

pub const ASSET_META_FILE_NAME: &str = "asset.yaml";

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),
    #[error("Invalid extension '{0}' in path {1}")]
    InvalidExtension(String, PathBuf),
    #[error("IoError: {0}")]
    IoError(#[from] io::Error),
    #[error("Extension already registered: {0}")]
    ExtensionAlreadyRegistered(String),
    #[error("Extension not registered: {0}")]
    ExtensionNotRegistered(String),
    #[error("Failed to start directory watcher in directory: {0}")]
    FailedToStartDirectoryWatcher(PathBuf),
    #[error("Failed to start thread pool")]
    FailedToStartThreadPool,
    #[error("Failed to read the asset: {0}")]
    InvalidAssetData(PathBuf),
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

pub(crate) fn extract_extension_from_path(path: &Path) -> Result<String> {
    Ok(path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_lowercase())
        .ok_or(Error::InvalidPath(path.to_owned()))?
        .to_owned())
}

pub(crate) fn extract_file_name_from_path(path: &Path) -> Result<String> {
    Ok(path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .ok_or(Error::InvalidPath(path.to_owned()))?
        .to_owned())
}

pub(crate) fn modified_system_time(path: &Path) -> Option<SystemTime> {
    path.metadata().ok().and_then(|metadata| metadata.modified().ok())
}
