use crate::{
    common::{extract_extension_from_path, AssetKey},
    read_asset::{FileSystem, FileSystemEvent, ImportSource},
    shader::{import_shader, ShaderAsset},
    Error, Result,
};
use jeriya_shared::{
    ahash::HashSet,
    bus::{Bus, BusReader},
    derive_where::derive_where,
    log::{error, info, trace},
    parking_lot::{Mutex, RwLock},
    rayon::{ThreadPool, ThreadPoolBuilder},
};
use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    marker::PhantomData,
    path::Path,
    sync::Arc,
};

pub type Importer<T> = dyn Fn(&[u8]) -> Result<T> + Send + Sync;

pub struct RawAsset {
    asset_key: AssetKey,
    _ty: TypeId,
    value: Mutex<Option<Arc<dyn Any + Send + Sync>>>,
}

#[derive_where(Clone)]
#[derive_where(crate = jeriya_shared::derive_where)]
pub struct Asset<T> {
    raw_asset: Arc<RawAsset>,
    _phantom: PhantomData<T>,
}

impl<T> Asset<T>
where
    T: 'static + Send + Sync,
{
    /// Returns the `AssetKey` of the asset.
    pub fn asset_key(&self) -> &AssetKey {
        &self.raw_asset.asset_key
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

type ImportFn = dyn for<'a> Fn(&AssetKey) + Send + Sync;

pub struct AssetImporter {
    thread_pool: Arc<ThreadPool>,

    /// Maps the file extension to the importer function.
    importers: Arc<Mutex<BTreeMap<String, Arc<ImportFn>>>>,

    /// Maps the type id to the channel that is used to send the result of the import. Any
    /// is used because the type of the channel depends on the type of the asset.
    asset_buses: Arc<Mutex<BTreeMap<TypeId, Box<dyn Any + Sync + Send>>>>,

    /// The bus that is used to send notifications when an asset was imported.
    notification_buses: Arc<Mutex<Bus<()>>>,

    importing_assets: Arc<RwLock<HashSet<AssetKey>>>,
    tracked_assets: Arc<RwLock<BTreeMap<AssetKey, Arc<RawAsset>>>>,
    import_source: Arc<RwLock<dyn ImportSource>>,
}

impl AssetImporter {
    /// Creates a new `AssetImporter`.
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_content::asset_importer::{AssetImporter, FileSystem};
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
        let importing_assets = Arc::new(RwLock::new(HashSet::default()));
        let importing_assets2 = importing_assets.clone();
        let thread_pool2 = thread_pool.clone();
        let watch_fn = move |event: FileSystemEvent| match event {
            FileSystemEvent::Create(path) => {
                trace!("Path '{}' was created", path.display());
                let asset_key = AssetKey::new(path);
                if let Err(err) = import(&asset_key, &thread_pool2, &importers2, &importing_assets2) {
                    error!("{err}");
                }
            }
            FileSystemEvent::Modify(path) => {
                trace!("Path '{}' was modified", path.display());
                let asset_key = AssetKey::new(path);
                if let Err(err) = import(&asset_key, &thread_pool2, &importers2, &importing_assets2) {
                    error!("{err}");
                }
            }
        };
        import_source.write().set_observer(Box::new(watch_fn))?;

        Ok(Self {
            thread_pool,
            importers,
            importing_assets,
            tracked_assets: Arc::new(RwLock::new(BTreeMap::new())),
            import_source,
            asset_buses: Arc::new(Mutex::new(BTreeMap::new())),
            notification_buses: Arc::new(Mutex::new(Bus::new(1024))),
        })
    }

    /// Creates a new `AssetImporter` with the default importers.
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_content::asset_importer::{AssetImporter, FileSystem};
    /// const ASSET_FOLDER: &str = "assets";
    /// std::fs::create_dir_all(ASSET_FOLDER).unwrap();
    /// let asset_importer = AssetImporter::default_from(ASSET_FOLDER).unwrap();
    /// ```
    pub fn default_from(root: impl AsRef<Path>) -> Result<Self> {
        let asset_source = FileSystem::new(root)?;
        Self::new(asset_source, 4).map(|asset_importer| {
            asset_importer
                .register::<ShaderAsset>("vert", Box::new(import_shader))
                .register::<ShaderAsset>("frag", Box::new(import_shader))
                .register::<ShaderAsset>("comp", Box::new(import_shader))
        })
    }

    /// Registers a new asset type.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::sync::Arc;
    /// use jeriya_content::{asset_importer::{AssetImporter, FileSystem}, Error};
    /// std::fs::create_dir_all("assets").unwrap();
    /// let asset_source = FileSystem::new("assets").unwrap();
    /// let mut asset_importer = AssetImporter::new(asset_source, 4).unwrap();
    ///
    /// asset_importer
    ///     .register::<String>(
    ///         "txt",
    ///         Box::new(|data| {
    ///             std::str::from_utf8(data)
    ///                 .map_err(|err| Error::Other(Box::new(err)))
    ///                 .map(|s| s.to_owned())
    ///         })
    ///     );
    /// ```
    pub fn register<T>(self, extension: impl Into<String>, importer: Box<Importer<T>>) -> Self
    where
        T: 'static + Send + Sync,
    {
        let extension = extension.into();

        let mut importers = self.importers.lock();
        if importers.contains_key(&extension) {
            panic!("importer for extension '{}' already registered", extension);
        }
        let tracked_assets2 = self.tracked_assets.clone();
        let import_source2 = self.import_source.clone();

        // Create bus to send the result of the import.
        let bus = Bus::<Arc<Result<Asset<T>>>>::new(1024);
        let mut buses = self.asset_buses.lock();
        buses.insert(TypeId::of::<T>(), Box::new(bus));
        drop(buses);
        let buses2 = self.asset_buses.clone();

        let notification_buses2 = self.notification_buses.clone();

        // Function to import an asset from a file.
        let import_from_file = move |asset_key: &AssetKey| -> Result<Asset<T>> {
            trace!("Reading meta data for asset '{asset_key}'");
            let meta_data = import_source2.read().read_meta_data(asset_key)?;
            info!("Meta data for asset '{asset_key}': {meta_data:#?}");

            trace!("Reading content for asset '{asset_key}'");
            let content = import_source2.read().read_content(asset_key, &meta_data.file)?;

            trace!("Starting the import for asset '{asset_key}'");
            let value = (importer)(&content)?;

            let raw_asset = Arc::new(RawAsset {
                asset_key: asset_key.clone(),
                _ty: TypeId::of::<T>(),
                value: Mutex::new(Some(Arc::new(value))),
            });
            if let Some(_) = tracked_assets2.write().insert(asset_key.clone(), raw_asset.clone()) {
                trace!("Tracked asset updated: {asset_key}");
            }
            Ok(Asset {
                raw_asset,
                _phantom: PhantomData,
            })
        };

        // Insert an import function into the map that does the import and sends the result to the receiver.
        importers.insert(
            extension.clone(),
            Arc::new(move |asset_key| {
                let result = import_from_file(asset_key);

                // Send the Asset to the receivers.
                let mut buses = buses2.lock();
                let bus = buses
                    .get_mut(&TypeId::of::<T>())
                    .and_then(|any| any.downcast_mut::<Bus<Arc<Result<Asset<T>>>>>())
                    .expect("failed to get bus for asset type although it must have been inserted at registration");
                bus.broadcast(Arc::new(result));

                // Send the notification to the receivers.
                let mut notification_buses = notification_buses2.lock();
                notification_buses.broadcast(());
            }),
        );
        drop(importers);
        info!("Registerd importer for extension '{extension}'");
        self
    }

    /// Returns the receiver for the given asset type when it was registered before.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use jeriya_content::{asset_importer::{AssetImporter, FileSystem}, Error};
    /// # std::fs::create_dir_all("assets").unwrap();
    /// # let asset_source = FileSystem::new("assets").unwrap();
    /// let mut asset_importer = AssetImporter::new(asset_source, 4)
    ///     .unwrap()
    ///     .register::<String>(
    ///          // snip
    /// #        "txt",
    /// #        Box::new(|data| {
    /// #            std::str::from_utf8(data)
    /// #                .map_err(|err| Error::Other(Box::new(err)))
    /// #                .map(|s| s.to_owned())
    /// #        })
    ///     );
    ///
    /// let receiver = asset_importer.receive_assets::<String>();
    /// assert!(receiver.is_some());
    /// ```
    pub fn receive_assets<T>(&self) -> Option<BusReader<Arc<Result<Asset<T>>>>>
    where
        T: 'static + Send + Sync,
    {
        let mut buses = self.asset_buses.lock();
        buses
            .get_mut(&TypeId::of::<T>())
            .and_then(|any| any.downcast_mut::<Bus<Arc<Result<Asset<T>>>>>())
            .map(|bus| bus.add_rx())
    }

    /// Returns the receiver of the bus that is used to send notifications when an asset was imported.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::{sync::Arc, fs::File, io::Write};
    /// # use jeriya_content::{asset_importer::{AssetImporter, FileSystem}, Error};
    /// # std::fs::create_dir_all("assets").unwrap();
    /// # let asset_source = FileSystem::new("assets").unwrap();
    /// let mut asset_importer = AssetImporter::new(asset_source, 4)
    ///     .unwrap()
    ///     .register::<String>(
    ///          // snip
    /// #        "txt",
    /// #        Box::new(|data| {
    /// #            std::str::from_utf8(data)
    /// #                .map_err(|err| Error::Other(Box::new(err)))
    /// #                .map(|s| s.to_owned())
    /// #        })
    ///     );
    ///
    /// let mut _receiver = asset_importer.receive_notifications();
    /// ```
    pub fn receive_notifications(&self) -> BusReader<()> {
        self.notification_buses.lock().add_rx()
    }

    /// Returns the asset with the given key when available in the tracked assets.
    ///
    /// # Example
    ///
    /// ```
    /// # use jeriya_content::{asset_importer::{AssetImporter, FileSystem}, common::AssetKey};
    /// # std::fs::create_dir_all("assets").unwrap();
    /// # let asset_source = FileSystem::new("assets").unwrap();
    /// let asset_importer = AssetImporter::new(asset_source, 4).unwrap();
    /// let manually_added = asset_importer.add(AssetKey::new("test.txt"), "Hello World!");
    /// let asset = asset_importer.get(&AssetKey::new("test.txt")).unwrap();
    /// assert_eq!(asset.value().unwrap(), manually_added.value().unwrap());
    /// ```
    pub fn get<T>(&self, asset_key: &AssetKey) -> Option<Asset<T>>
    where
        T: 'static + Send + Sync,
    {
        self.tracked_assets.read().get(asset_key).map(|raw_asset| Asset {
            raw_asset: raw_asset.clone(),
            _phantom: PhantomData,
        })
    }

    /// Adds an asset to the tracked assets.
    ///
    /// # Example
    ///
    /// ```
    /// # use jeriya_content::{asset_importer::{AssetImporter, FileSystem}, common::AssetKey};
    /// # std::fs::create_dir_all("assets").unwrap();
    /// # let asset_source = FileSystem::new("assets").unwrap();
    /// let asset_importer = AssetImporter::new(asset_source, 4).unwrap();
    /// let _manually_added = asset_importer.add(AssetKey::new("test.txt"), "Hello World!");
    /// ```
    pub fn add<T>(&self, asset_key: AssetKey, value: T) -> Asset<T>
    where
        T: 'static + Send + Sync,
    {
        let raw_asset = Arc::new(RawAsset {
            asset_key: asset_key.clone(),
            _ty: TypeId::of::<T>(),
            value: Mutex::new(Some(Arc::new(value))),
        });
        self.tracked_assets.write().insert(asset_key.clone(), raw_asset.clone());
        Asset {
            raw_asset,
            _phantom: PhantomData,
        }
    }

    /// Imports all assets of the given type.
    pub fn import_all<T>(&self) -> Result<()> {
        todo!()
    }

    /// Imports an asset from the given path.
    pub fn import<T>(&self, asset_key: impl Into<AssetKey>) -> Result<()> {
        import(&asset_key.into(), &self.thread_pool, &self.importers, &self.importing_assets)
    }
}

fn import(
    asset_key: &AssetKey,
    thread_pool: &ThreadPool,
    importers: &Arc<Mutex<BTreeMap<String, Arc<ImportFn>>>>,
    importing_assets: &Arc<RwLock<HashSet<AssetKey>>>,
) -> Result<()> {
    let importers = importers.clone();

    trace!("Extracting extension from '{asset_key}'");
    let extension = extract_extension_from_path(asset_key.as_path())?;

    trace!("Checking if the extension '{extension}' is registered");
    let guard = importers.lock();
    if !guard.contains_key(&extension) {
        return Err(Error::ExtensionNotRegistered(extension));
    }
    drop(guard);

    trace!("Checking if the asset '{asset_key}' is already being imported");
    let mut guard = importing_assets.write();
    if guard.contains(asset_key) {
        return Ok(());
    }
    guard.insert(asset_key.clone());
    drop(guard);

    let importing_assets2 = importing_assets.clone();

    // Spawn a thread to import the asset.
    let asset_key = asset_key.clone();
    thread_pool.spawn(move || {
        let importers = importers.lock();
        let importer = importers
            .get(&extension)
            // The import function checks if the extension is registered and since there is way to
            // remove an extension, this should never fail.
            .expect("failed to find the configuration for the given extension")
            .clone();
        importer(&asset_key);

        trace!("Removing asset '{asset_key}' from the importing assets");
        let mut importing_assets = importing_assets2.write();
        importing_assets.remove(&asset_key);
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use jeriya_shared::indoc::indoc;
    use jeriya_test::setup_logger;
    use tempdir::TempDir;

    use crate::common::ASSET_META_FILE_NAME;

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

    fn expect_asset<T>(result: std::result::Result<Arc<Result<Asset<T>>>, std::sync::mpsc::RecvTimeoutError>) -> Asset<T> {
        match result.unwrap().as_ref() {
            Ok(asset) => asset.clone(),
            Err(err) => panic!("Failed to import asset: {:#?}", err),
        }
    }

    #[test]
    fn smoke() {
        setup_logger();

        let root = TempDir::new("root").unwrap();

        create_processed_asset(root.path(), "Hello World!");

        let asset_source = FileSystem::new(root.path().to_owned()).unwrap();
        let asset_importer = AssetImporter::new(asset_source, 4).unwrap().register::<String>(
            "txt",
            Box::new(|data| {
                std::str::from_utf8(data)
                    .map_err(|err| Error::Other(Box::new(err)))
                    .map(|s| s.to_owned())
            }),
        );
        let mut receiver = asset_importer.receive_assets::<String>().unwrap();

        // Start the import process.
        asset_importer.import::<String>("test.txt").unwrap();

        // Receive and check the result.
        let result = expect_asset(receiver.recv_timeout(Duration::from_millis(100)));
        assert_eq!(*result.value().unwrap(), "Hello World!");
        assert_eq!(result.asset_key().as_path(), Path::new("test.txt"));
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
        let asset_importer = AssetImporter::new(asset_source, 4).unwrap().register::<String>(
            "txt",
            Box::new(|data| {
                std::str::from_utf8(data)
                    .map_err(|err| Error::Other(Box::new(err)))
                    .map(|s| s.to_owned())
            }),
        );
        let mut receiver = asset_importer.receive_assets::<String>().unwrap();

        // Update the file content
        create_processed_asset(root.path(), "Hello World 2!");

        // Receive and check the result.
        let asset = expect_asset(receiver.recv_timeout(Duration::from_millis(1000)));
        assert_eq!(asset.value(), Some(Arc::new("Hello World 2!".to_owned())));
    }

    #[test]
    fn receive_notification_and_asset() {
        setup_logger();

        let root = TempDir::new("root").unwrap();

        let asset_source = FileSystem::new(root.path()).unwrap();
        let mut asset_importer = AssetImporter::new(asset_source, 4).unwrap().register::<String>(
            "txt",
            Box::new(|data| {
                std::str::from_utf8(data)
                    .map_err(|err| Error::Other(Box::new(err)))
                    .map(|s| s.to_owned())
            }),
        );

        let mut notification_receiver = asset_importer.receive_notifications();
        let mut asset_receiver = asset_importer.receive_assets::<String>().unwrap();

        // Write the asset
        create_processed_asset(root.path(), "Hello World!");

        // Expect the notification
        let result = notification_receiver.recv_timeout(Duration::from_millis(1000));
        assert!(result.is_ok());

        // Expect the asset
        let asset = expect_asset(asset_receiver.recv_timeout(Duration::from_millis(1000)));
        assert_eq!(asset.value(), Some(Arc::new("Hello World!".to_owned())));
    }
}
