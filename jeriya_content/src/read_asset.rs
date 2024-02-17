use crate::{
    common::{extract_file_name_from_path, AssetKey, ASSET_META_FILE_NAME},
    Error, Result,
};
use jeriya_shared::{
    log::{error, info, warn},
    pathdiff,
};
use notify_debouncer_full::notify::{self, EventKind, ReadDirectoryChangesWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    result,
};

pub enum FileSystemEvent {
    Create(PathBuf),
    Modify(PathBuf),
}

pub type ObserverFn = dyn Fn(FileSystemEvent) + Send + Sync;

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMetaData {
    pub file: PathBuf,
}

pub trait ReadAsset {
    /// Read the [`AssetMetaData`] from the given asset path.
    fn read_meta_data(&self, asset_key: &AssetKey) -> Result<AssetMetaData>;

    /// Read the content of the file that belongs to the given `asset_key`.
    fn read_content(&self, asset_key: &AssetKey, file_path: &Path) -> Result<Vec<u8>>;
}

pub trait ImportSource: ReadAsset + Send + Sync {
    /// Sets the observer function that is called when an asset is created or modified.
    fn set_observer(&mut self, observer_fn: Box<ObserverFn>) -> Result<()>;
}

pub struct FileSystem {
    root: PathBuf,
    watcher: Option<ReadDirectoryChangesWatcher>,
}

impl FileSystem {
    /// Creates a new [`FileSystem`] import source and checks that the given root directory exists.
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_content::asset_importer::FileSystem;
    /// std::fs::create_dir_all("assets").unwrap();
    /// let _file_system = FileSystem::new("assets").unwrap();
    /// ```
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = env::current_dir().expect("unable to determine the working directory").join(root);
        if !root.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("Directory '{}' does not exist", root.display())).into());
        }
        Ok(Self { root, watcher: None })
    }
}

fn check_path(asset_key: &AssetKey) -> Result<()> {
    if asset_key.as_path().is_absolute() {
        return Err(Error::InvalidPath(asset_key.as_path().to_owned()));
    }
    Ok(())
}

fn is_meta_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(file_name) = extract_file_name_from_path(path) else {
        error!("Failed to extract file name from '{path}'", path = path.display());
        return false;
    };
    file_name == ASSET_META_FILE_NAME
}

impl ReadAsset for FileSystem {
    fn read_meta_data(&self, asset_key: &AssetKey) -> Result<AssetMetaData> {
        check_path(asset_key)?;
        let meta_file_path = self.root.join(asset_key.as_path()).join(ASSET_META_FILE_NAME);
        let meta_file_content = fs::read_to_string(&meta_file_path)?;
        let meta_data = serde_yaml::from_str(&meta_file_content).map_err(|_| Error::InvalidAssetData(meta_file_path.clone()))?;
        Ok(meta_data)
    }

    fn read_content(&self, asset_key: &AssetKey, file_path: &Path) -> Result<Vec<u8>> {
        check_path(asset_key)?;
        let path = self.root.join(asset_key.as_path()).join(file_path);
        fs::read(&path).map_err(|_| Error::InvalidAssetData(path))
    }
}

impl ImportSource for FileSystem {
    /// Sets the observer function that is called when an asset is created or modified.
    ///
    /// # Panics
    ///
    /// If the `observer_fn` is already set.
    fn set_observer(&mut self, observer_fn: Box<ObserverFn>) -> Result<()> {
        if self.watcher.is_some() {
            panic!("set_observer called although the observer is already set");
        }
        let root = self.root.clone();

        let watch_fn = move |result: result::Result<notify::Event, notify::Error>| match result {
            Ok(event) => {
                let absolute_path = event.paths.first().expect("no path in event");

                // Only changes in the meta file are relevant because there might be more than
                // one file per asset and we only want to react to the change once. It is expected
                // that the meta file is always changed or created last.
                if !is_meta_file(absolute_path) {
                    return;
                }

                // The file watcher returns absolute paths but he whole asset handling is based on
                // relative paths because it's irrelavant where on the system they are located.
                let Some(path) = pathdiff::diff_paths(absolute_path, &root) else {
                    warn! {
                        "Failed to get relative path of '{absolute_path}' relative to '{root}'",
                        absolute_path = absolute_path.display(),
                        root = root.display()
                    };
                    return;
                };
                assert!(path.is_relative(), "path '{}' is not relative", path.display());

                // The asset path is the parent of the meta file.
                let asset_path = path.parent().expect("path has no parent");

                if let EventKind::Modify(_modify_event) = &event.kind {
                    info!("Emitting modify event for asset '{}'", path.display());
                    observer_fn(FileSystemEvent::Modify(asset_path.to_owned()))
                }
            }
            Err(_) => todo!(),
        };

        // Start the directory watcher.
        let mut watcher = notify::recommended_watcher(watch_fn).map_err(|_| Error::FailedToStartDirectoryWatcher(self.root.clone()))?;
        watcher
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(|_| Error::FailedToStartDirectoryWatcher(self.root.clone()))?;
        self.watcher = Some(watcher);

        Ok(())
    }
}
