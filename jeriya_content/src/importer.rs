use crate::{
    common::{extract_extension_from_path, extract_file_name_from_path, ASSET_META_FILE_NAME},
    Error, Result,
};
use jeriya_shared::{
    crossbeam_channel::{self, Receiver},
    log::{error, info, trace, warn},
    parking_lot::{Mutex, RwLock},
    pathdiff,
    rayon::{ThreadPool, ThreadPoolBuilder},
};
use notify_debouncer_full::notify::{self, EventKind, ReadDirectoryChangesWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    env, fs, io,
    marker::PhantomData,
    path::{Path, PathBuf},
    result,
    sync::Arc,
};

pub type Importer<T> = dyn Fn(&[u8]) -> Result<T> + Send + Sync;

pub type ObserverFn = dyn Fn(Event) + Send + Sync;

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMetaData {
    pub file: PathBuf,
}

pub enum Event {
    Create(PathBuf),
    Modify(PathBuf),
}

pub trait ReadAsset {
    /// Read the [`AssetMetaData`] from the given asset path.
    fn read_meta_data(&self, asset_path: &Path) -> Result<AssetMetaData>;

    /// Read the content of the file that belongs to the given `asset_path`.
    fn read_content(&self, asset_path: &Path, file_path: &Path) -> Result<Vec<u8>>;
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
    /// use jeriya_content::FileSystem;
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

fn check_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(Error::InvalidPath(path.to_owned()));
    }
    Ok(())
}

impl ReadAsset for FileSystem {
    fn read_meta_data(&self, asset_path: &Path) -> Result<AssetMetaData> {
        check_path(asset_path)?;
        let meta_file_path = self.root.join(asset_path).join(ASSET_META_FILE_NAME);
        let meta_file_content = fs::read_to_string(&meta_file_path)?;
        let meta_data = serde_yaml::from_str(&meta_file_content).map_err(|_| Error::InvalidAssetData(meta_file_path.clone()))?;
        Ok(meta_data)
    }

    fn read_content(&self, asset_path: &Path, file_path: &Path) -> Result<Vec<u8>> {
        check_path(asset_path)?;
        let path = self.root.join(asset_path).join(file_path);
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
                if !is_meta_file(&absolute_path) {
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

                match &event.kind {
                    EventKind::Modify(_modify_event) => {
                        info!("Emitting modify event for asset '{}'", path.display());
                        observer_fn(Event::Modify(asset_path.to_owned()))
                    }
                    // We don't care about other events like Create because we would handle multiple
                    // events per processing operation of an asset: one when the file is created and
                    // one when the contents of the file are written.
                    _ => {}
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

pub struct RawAsset {
    path: PathBuf,
    ty: TypeId,
    value: Mutex<Option<Arc<dyn Any + Send + Sync>>>,
}

pub struct Asset<T> {
    raw_asset: Arc<RawAsset>,
    _phantom: PhantomData<T>,
}

impl<T> Asset<T>
where
    T: 'static + Send + Sync,
{
    /// Returns the path of the asset.
    pub fn path(&self) -> &Path {
        &self.raw_asset.path
    }

    /// Drops the data of the asset but keeps it as a tracked asset.
    pub fn drop_data(&self) {
        *self.raw_asset.value.lock() = None;
    }

    /// Returns the actual value of the `Asset<T>`.
    pub fn value(&self) -> Option<Arc<T>> {
        self.raw_asset
            .value
            .lock()
            .as_ref()
            .map(|value| value.clone().downcast::<T>().expect("type mismatch"))
    }
}

pub struct ImportConfiguration<T>
where
    T: 'static + Send + Sync,
{
    /// Extension of the files that should be imported with the given [`Importer`].
    pub extension: String,
    /// The [`Importer`] function that converts the raw data into the asset type.
    pub importer: Box<Importer<T>>,
}

type ImportFn = dyn for<'a> Fn(&Path) + Send + Sync;

pub struct AssetImporter {
    thread_pool: Arc<ThreadPool>,

    /// Maps the file extension to the importer function.
    importers: Arc<Mutex<BTreeMap<String, Arc<ImportFn>>>>,

    tracked_assets: Arc<RwLock<BTreeMap<PathBuf, Arc<RawAsset>>>>,
    import_source: Arc<RwLock<dyn ImportSource>>,
}

impl AssetImporter {
    /// Creates a new `AssetImporter`.
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_content::{AssetImporter, FileSystem};
    /// std::fs::create_dir_all("assets").unwrap();
    /// let asset_source = FileSystem::new("assets").unwrap();
    /// let asset_importer = AssetImporter::new(asset_source, 4).unwrap();
    /// ```
    pub fn new<I>(import_source: I, num_threads: usize) -> Result<Self>
    where
        I: ImportSource + 'static,
    {
        info!("Create thread pool with {} threads for AssetImporter", num_threads);
        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .map(Arc::new)
            .map_err(|_| Error::FailedToStartThreadPool)?;

        let import_source = Arc::new(RwLock::new(import_source));

        info!("Set the observer function for the import source");
        let importers = Arc::new(Mutex::new(BTreeMap::new()));
        let importers2 = importers.clone();
        let thread_pool2 = thread_pool.clone();
        let watch_fn = move |event: Event| match event {
            Event::Create(path) => {
                trace!("Path '{}' was created", path.display());
                if let Err(err) = import(path, &thread_pool2, &importers2) {
                    error!("{err}");
                }
            }
            Event::Modify(path) => {
                trace!("Path '{}' was modified", path.display());
                if let Err(err) = import(path, &thread_pool2, &importers2) {
                    error!("{err}");
                }
            }
        };
        import_source.write().set_observer(Box::new(watch_fn))?;

        Ok(Self {
            thread_pool,
            importers,
            tracked_assets: Arc::new(RwLock::new(BTreeMap::new())),
            import_source,
        })
    }

    /// Registers a new asset type.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::sync::Arc;
    /// use jeriya_content::{AssetImporter, FileSystem, ImportConfiguration, Error};
    /// std::fs::create_dir_all("assets").unwrap();
    /// let asset_source = FileSystem::new("assets").unwrap();
    /// let mut asset_importer = AssetImporter::new(asset_source, 4).unwrap();
    ///
    /// asset_importer
    ///     .register::<String>(ImportConfiguration {
    ///         extension: "txt".to_owned(),
    ///         importer: Box::new(|data| {
    ///             std::str::from_utf8(data)
    ///                 .map_err(|err| Error::Other(Box::new(err)))
    ///                 .map(|s| s.to_owned())
    ///         }),
    ///     })
    ///     .unwrap();
    /// ```
    pub fn register<T>(&mut self, import_configuration: ImportConfiguration<T>) -> Result<Receiver<Result<Arc<Asset<T>>>>>
    where
        T: 'static + Send + Sync,
    {
        let mut importers = self.importers.lock();
        if importers.contains_key(&import_configuration.extension) {
            return Err(Error::ExtensionAlreadyRegistered(import_configuration.extension.clone()));
        }
        let extension = import_configuration.extension.clone();
        let tracked_assets2 = self.tracked_assets.clone();
        let import_source2 = self.import_source.clone();
        let (sender, receiver) = crossbeam_channel::unbounded();

        // Function to import an asset from a file.
        let import_from_file = move |path: &Path| -> Result<Arc<Asset<T>>> {
            trace!("Reading meta data for asset '{}'", path.display());
            let meta_data = import_source2.read().read_meta_data(path)?;
            info!("Meta data for asset '{}': {:#?}", path.display(), &meta_data);

            trace!("Reading content for asset '{}'", path.display());
            let content = import_source2.read().read_content(path, &meta_data.file)?;

            trace!("Starting the import for asset '{}'", path.display());
            let value = (import_configuration.importer)(&content)?;

            let raw_asset = Arc::new(RawAsset {
                path: path.to_owned(),
                ty: TypeId::of::<T>(),
                value: Mutex::new(Some(Arc::new(value))),
            });
            tracked_assets2.write().insert(path.to_owned(), raw_asset.clone());
            let asset = Arc::new(Asset {
                raw_asset,
                _phantom: PhantomData,
            });

            Ok(asset)
        };

        // Insert an import function into the map that does the import and sends the result to the receiver.
        importers.insert(
            extension.clone(),
            Arc::new(move |path| {
                let result = import_from_file(path);
                let result_string = match &result {
                    Ok(..) => format!("Ok"),
                    Err(..) => format!("Err"),
                };
                match sender.send(result) {
                    Ok(()) => info!("Successfully imported asset '{}'", path.display()),
                    Err(err) => {
                        error!(
                            "Failed to send result '{result_string}' of import for asset '{}': {err:?}",
                            path.display()
                        )
                    }
                }
            }),
        );
        info!("Registerd importer for extension '{extension}'");
        Ok(receiver)
    }

    /// Imports all assets of the given type.
    pub fn import_all<T>(&self) -> Result<()> {
        todo!()
    }

    /// Imports an asset from the given path.
    pub fn import<T>(&self, path: impl AsRef<Path>) -> Result<()> {
        import(path, &self.thread_pool, &self.importers)
    }
}

fn is_meta_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(file_name) = extract_file_name_from_path(&path) else {
        error!("Failed to extract file name from '{path}'", path = path.display());
        return false;
    };
    file_name == ASSET_META_FILE_NAME
}

fn import(import_path: impl AsRef<Path>, thread_pool: &ThreadPool, importers: &Arc<Mutex<BTreeMap<String, Arc<ImportFn>>>>) -> Result<()> {
    let importers = importers.clone();
    let path = import_path.as_ref().to_owned();

    trace!("Extracting extension from path '{path:?}'");
    let extension = extract_extension_from_path(&path)?;

    trace!("Checking if the extension '{extension}' is registered");
    if !importers.lock().contains_key(&extension) {
        return Err(Error::ExtensionNotRegistered(extension));
    }

    // Spawn a thread to import the asset.
    thread_pool.spawn(move || {
        let importers = importers.lock();
        let importer = importers
            .get(&extension)
            // The import function checks if the extension is registered and since there is way to
            // remove an extension, this should never fail.
            .expect("failed to find the configuration for the given extension")
            .clone();
        importer(&path.to_owned());
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jeriya_shared::indoc::indoc;
    use jeriya_test::setup_logger;
    use tempdir::TempDir;

    use super::*;

    /// Creates a sample asset containing only a string.
    fn create_processed_asset(root: &Path, content: &str) {
        let root = root.to_owned();

        // This is the folder in the target directory where the processed assets are
        // stored. Therefore, it has the same name as the original asset.
        const ASSET_FOLDER_NAME: &str = "test.txt";

        // This is the filename of the file that contains the actual content of the
        // processed asset. In this case it is just a text file with a `.bin` extension.
        const ASSET_CONTENT_FILE_NAME: &str = "test.bin";

        // Remove and create directory
        let asset_folder = root.join(ASSET_FOLDER_NAME);
        let _ = fs::remove_dir_all(&asset_folder);
        fs::create_dir_all(&asset_folder).unwrap();

        // Create content file which contains the actual string data. Generally, this
        // has to be done before the asset meta file is created because the hot reload
        // is triggered when the meta file is created or modfied.
        let asset_content_file = asset_folder.join(ASSET_CONTENT_FILE_NAME);
        fs::write(asset_content_file, &content).unwrap();

        // Create the meta file which contains the information about the asset.
        let meta_file = asset_folder.join(ASSET_META_FILE_NAME);
        let meta_file_content = indoc! {"
            file: \"test.bin\" # Determines the file where to find the actual data
        "};
        fs::write(meta_file, meta_file_content).unwrap();
    }

    #[test]
    fn create_sample_asset() {
        let root = TempDir::new("root").unwrap();
        create_processed_asset(root.path(), "Hello World!");
        assert!(root.path().to_owned().join("test.txt").is_dir());
        assert!(root.path().to_owned().join("test.txt/test.bin").is_file());
        assert!(root.path().to_owned().join("test.txt/asset.yaml").is_file());
    }

    #[test]
    fn smoke() {
        setup_logger();

        let root = TempDir::new("root").unwrap();

        create_processed_asset(root.path(), "Hello World!");

        let asset_source = FileSystem::new(root.path().to_owned()).unwrap();
        let mut asset_importer = AssetImporter::new(asset_source, 4).unwrap();

        // Importer that converts a text file to a `String`.
        let receiver = asset_importer
            .register::<String>(ImportConfiguration {
                extension: "txt".to_owned(),
                importer: Box::new(|data| {
                    std::str::from_utf8(data)
                        .map_err(|err| Error::Other(Box::new(err)))
                        .map(|s| s.to_owned())
                }),
            })
            .unwrap();

        // Start the import process.
        asset_importer.import::<String>("test.txt").unwrap();

        // Receive and check the result.
        let result = receiver.recv_timeout(Duration::from_millis(100)).unwrap().unwrap();
        assert_eq!(*result.value().unwrap(), "Hello World!");
        assert_eq!(result.path(), Path::new("test.txt"));
        assert_eq!(result.value(), Some(Arc::new("Hello World!".to_owned())));
        result.drop_data();
        assert_eq!(result.value(), None);
    }

    #[test]
    fn directory_not_found() {
        let asset_source = FileSystem::new("not_found");
        assert!(asset_source.is_err());
    }

    #[test]
    fn hot_reload() {
        setup_logger();

        info!("test");

        let root = TempDir::new("root").unwrap();

        create_processed_asset(root.path(), "Hello World!");

        let asset_source = FileSystem::new(root.path().to_owned()).unwrap();
        let mut asset_importer = AssetImporter::new(asset_source, 4).unwrap();

        // Importer that converts a text file to a `String`.
        let receiver = asset_importer
            .register::<String>(ImportConfiguration {
                extension: "txt".to_owned(),
                importer: Box::new(|data| {
                    std::str::from_utf8(data)
                        .map_err(|err| Error::Other(Box::new(err)))
                        .map(|s| s.to_owned())
                }),
            })
            .unwrap();

        // Update the file content
        create_processed_asset(root.path(), "Hello World 2!");

        // Receive and check the result.
        let asset = receiver.recv_timeout(Duration::from_millis(1000)).unwrap().unwrap();
        assert_eq!(asset.value(), Some(Arc::new("Hello World 2!".to_owned())));
    }
}
