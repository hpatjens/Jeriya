use std::{
    borrow::Cow,
    fmt::{self, Formatter},
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

/// Identifies the asset. It's a relative path in the asset directory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetKey(PathBuf);

impl AssetKey {
    /// Create a new [`AssetKey`] from a path. No validation is done on the path.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jeriya_content::AssetKey;
    /// let asset_key = AssetKey::new("textures/character.png");
    /// assert_eq!(asset_key.as_str(), "textures/character.png");
    /// ```
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the path of the asset.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::ffi::OsStr;
    /// use jeriya_content::AssetKey;
    /// let asset_key = AssetKey::new("textures/character.png");
    /// assert_eq!(asset_key.as_path().extension(), Some(OsStr::new("png")));
    /// ```
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Returns the path of the asset.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::ffi::OsStr;
    /// use jeriya_content::AssetKey;
    /// let asset_key = AssetKey::new("textures/character.png");
    /// assert_eq!(asset_key.as_str(), "textures/character.png");
    /// ```
    pub fn as_str(&self) -> Cow<str> {
        self.0.to_string_lossy()
    }
}

impl fmt::Display for AssetKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "AssetKey({})", self.as_str())
    }
}

impl From<&str> for AssetKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<&Path> for AssetKey {
    fn from(value: &Path) -> Self {
        Self::new(value)
    }
}

impl From<&AssetKey> for AssetKey {
    fn from(value: &AssetKey) -> Self {
        value.clone()
    }
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
