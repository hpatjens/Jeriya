use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{
    common::{extract_extension_from_path, modified_system_time, ASSET_META_FILE_NAME},
    AssetKey, Error, Result,
};
use jeriya_shared::{
    crossbeam_channel::{self, Receiver, Sender},
    log::{error, info, trace, warn},
    parking_lot::Mutex,
    pathdiff,
    rayon::{ThreadPool, ThreadPoolBuilder},
    walkdir::WalkDir,
};
use notify_debouncer_full::{
    notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher},
    DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};

type ProcessFn = dyn Fn(&AssetKey, &Path, &Path) + Send + Sync;

pub type Processor = dyn Fn(&mut AssetBuilder) -> Result<()> + Send + Sync;

pub struct ProcessConfiguration {
    pub extension: String,
    pub processor: Box<Processor>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Processed(PathBuf),
}

/// Directories that are used by the [`AssetProcessor`].
#[derive(Debug, Clone)]
pub struct Directories {
    unprocessed_assets_path: PathBuf,
    processed_assets_path: PathBuf,
}

impl Directories {
    /// Creates the directories that are used by the [`AssetProcessor`].
    pub fn create_all_dir(unprocessed_assets_path: impl AsRef<Path>, processed_assets_path: impl AsRef<Path>) -> io::Result<Directories> {
        trace!("Creating directory for unprocessed assets: {:?}", unprocessed_assets_path.as_ref());
        fs::create_dir_all(&unprocessed_assets_path)?;
        trace!("Creating directory for processed assets: {:?}", processed_assets_path.as_ref());
        fs::create_dir_all(&processed_assets_path)?;
        let unprocessed_assets_path = unprocessed_assets_path
            .as_ref()
            .canonicalize()
            .expect("failed to canonicalize path to the unprocessed assets")
            .to_path_buf();
        let processed_assets_path = processed_assets_path
            .as_ref()
            .canonicalize()
            .expect("failed to canonicalize path to the processed assets")
            .to_path_buf();
        let result = Self {
            unprocessed_assets_path,
            processed_assets_path,
        };
        assert!(result.check().is_ok());
        Ok(result)
    }

    /// Returns `true` if the directories exist.
    pub fn exist(&self) -> bool {
        self.unprocessed_assets_path.exists() && self.processed_assets_path.exists()
    }

    /// Assets that the directories exist and returns a specific error if they don't.
    pub fn check(&self) -> Result<()> {
        if !self.processed_assets_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Directory for processed assets '{}' does not exist",
                    self.processed_assets_path.display()
                ),
            )
            .into());
        }
        if !self.unprocessed_assets_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Directory for unprocessed assets '{}' does not exist",
                    self.unprocessed_assets_path.display()
                ),
            )
            .into());
        }
        Ok(())
    }

    /// Returns the path to the directory where the unprocessed assets are located.
    pub fn unprocessed_assets_path(&self) -> &Path {
        &self.unprocessed_assets_path
    }

    /// Returns the path to the directory where the processed assets are located.
    pub fn processed_assets_path(&self) -> &Path {
        &self.processed_assets_path
    }
}

pub struct AssetProcessor {
    running: Arc<AtomicBool>,
    senders: Arc<Mutex<Vec<Sender<Event>>>>,
    processors: Arc<Mutex<BTreeMap<String, Arc<ProcessFn>>>>,
    _watcher: Debouncer<RecommendedWatcher, FileIdMap>,
}

impl AssetProcessor {
    /// Creates a new [`AssetProcessor`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use jeriya_content::{AssetProcessor, Directories};
    /// std::fs::create_dir_all("unprocessed").unwrap();
    /// std::fs::create_dir_all("processed").unwrap();
    /// let directories = Directories::create_all_dir("unprocessed", "processed").unwrap();
    /// let asset_processor = AssetProcessor::new(&directories, 4).unwrap();
    /// ```
    pub fn new(directories: &Directories, num_threads: usize) -> crate::Result<Self> {
        let directories = directories.clone();
        directories.check()?;
        info!("Creating AssetProcessor for '{directories:?}'");

        let senders = Arc::new(Mutex::new(Vec::new()));
        let processors = Arc::new(Mutex::new(BTreeMap::new()));

        // The [`AssetProcessor`] has to be started manually after the constructor has run so
        // that the user can register processors and receive events for all assets.
        let running = Arc::new(AtomicBool::new(false));

        info!("Create thread pool with {} threads for AssetProcessor", num_threads);
        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .map(Arc::new)
            .map_err(|_| Error::FailedToStartThreadPool)?;

        let running2 = running.clone();
        let processors2 = processors.clone();
        let thread_pool2 = thread_pool.clone();
        let directories2 = directories.clone();
        let senders2 = senders.clone();
        let watch_fn = move |result: DebounceEventResult| match result {
            Ok(events) => {
                for event in events {
                    let absolute_path = event.paths.first().expect("Event has no path");
                    assert!(absolute_path.is_absolute(), "path '{}' is not absolute", absolute_path.display());

                    // Check if the processor is active.
                    let event_name = event_name(&event);
                    if !running2.load(Ordering::SeqCst) {
                        info! {
                            "Watcher is inactive and reported '{event_name}' event for path '{}'",
                            absolute_path.display()
                        }
                        return;
                    } else {
                        info! {
                            "Watcher is active and reported '{event_name}' event for path '{}'",
                            absolute_path.display()
                        }
                    }

                    let thread_pool = thread_pool2.clone();
                    let processors = processors2.clone();

                    // The file watcher returns absolute paths but he whole asset handling is based on
                    // relative paths because it's irrelavant where on the system they are located.
                    let Some(path) = pathdiff::diff_paths(&absolute_path, &directories2.unprocessed_assets_path()) else {
                        warn! {
                            "Failed to get relative path of '{absolute_path}' relative to '{unprocessed_assets_path}'", 
                            absolute_path = absolute_path.display(),
                            unprocessed_assets_path = directories2.unprocessed_assets_path().display()
                        };
                        return;
                    };
                    assert!(path.is_relative(), "path '{}' is not relative", path.display());
                    let asset_key = AssetKey::new(path);

                    match &event.kind {
                        EventKind::Create(_create_event) => {
                            if let Err(err) = process(&asset_key, &directories2, &thread_pool, &processors, &senders2) {
                                error!("Failed to process file '{asset_key}': {err}");
                            }
                        }
                        EventKind::Modify(_modify_event) => {
                            if let Err(err) = process(&asset_key, &directories2, &thread_pool, &processors, &senders2) {
                                error!("Failed to process file '{asset_key}': {err}");
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(_) => {}
        };

        run_inventory(&directories, &thread_pool, &processors, &senders)?;

        // Start the directory watcher.
        let mut watcher = notify_debouncer_full::new_debouncer(Duration::from_millis(1000), None, watch_fn)
            .map_err(|_| Error::FailedToStartDirectoryWatcher(directories.unprocessed_assets_path().to_owned()))?;
        watcher
            .watcher()
            .watch(directories.unprocessed_assets_path(), RecursiveMode::Recursive)
            .map_err(|_| Error::FailedToStartDirectoryWatcher(directories.unprocessed_assets_path().to_owned()))?;

        Ok(Self {
            running,
            senders,
            processors,
            _watcher: watcher,
        })
    }

    /// Either sets the [`AssetProcessor`] to active or inactive.
    pub fn set_active(&self, active: bool) {
        self.running.store(active, Ordering::SeqCst);
    }

    /// Registers a [`Processor`] for the given file extension.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jeriya_content::{AssetProcessor, ProcessConfiguration, Directories};
    /// let directories = Directories::create_all_dir("unprocessed", "processed").unwrap();
    /// let mut asset_processor = AssetProcessor::new(&directories, 4).unwrap();
    ///
    /// asset_processor
    ///     .register(ProcessConfiguration {
    ///         extension: "txt".to_owned(),
    ///         processor: Box::new(|asset_builder| {
    ///             let content = std::fs::read_to_string(asset_builder.unprocessed_asset_path()).unwrap();
    ///             let processed_content = content.replace("World", "Universe");
    ///             std::fs::write(asset_builder.processed_asset_path().join("test.bin"), processed_content).unwrap();
    ///             Ok(())
    ///         }),
    ///     })
    ///     .unwrap();
    /// ```
    pub fn register(&mut self, process_config: ProcessConfiguration) -> Result<()> {
        let mut processors = self.processors.lock();
        if processors.contains_key(&process_config.extension) {
            return Err(Error::ExtensionAlreadyRegistered(process_config.extension.clone()));
        }

        processors.insert(
            process_config.extension,
            Arc::new(move |asset_key, unprocessed_asset_path, processed_asset_path| {
                info!("Processing file: {asset_key}");
                let mut asset_builder = AssetBuilder::new(asset_key, unprocessed_asset_path, processed_asset_path);
                let process_result = (process_config.processor)(&mut asset_builder);
                let asset_write_result = asset_builder.build();
                match process_result.or(asset_write_result) {
                    Ok(()) => info!("Successfully processed file: {asset_key}"),
                    Err(err) => error!("Failed to process file '{asset_key}': {err}"),
                }
            }),
        );

        Ok(())
    }

    /// Returns a channel that can be used to observe [`Event`]s.
    pub fn observe(&mut self) -> Receiver<Event> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        self.senders.lock().push(sender);
        receiver
    }
}

fn event_name(event: &DebouncedEvent) -> &str {
    match &event.kind {
        EventKind::Any => "Any",
        EventKind::Access(_) => "Access",
        EventKind::Create(_) => "Create",
        EventKind::Modify(_) => "Modify",
        EventKind::Remove(_) => "Remove",
        EventKind::Other => "Other",
    }
}

fn process(
    asset_key: &AssetKey,
    directories: &Directories,
    thread_pool: &ThreadPool,
    processors: &Arc<Mutex<BTreeMap<String, Arc<ProcessFn>>>>,
    senders: &Arc<Mutex<Vec<Sender<Event>>>>,
) -> Result<()> {
    let processors = processors.clone();
    let asset_path = asset_key.as_path().to_owned();

    assert! {
        asset_path.is_relative(),
        "asset_key must be relative so that it can be used in the unprocessed and processed directories"
    }
    assert! {
        asset_path.extension().is_some(),
        "asset_key must have an extension so that the AssetImporter can determine the correct importer"
    }

    trace!("Extracing extension from path: {asset_path:?}");
    let extension = extract_extension_from_path(&asset_path)?;

    trace!("Checking if the extension '{extension}' is registered");
    if !processors.lock().contains_key(&extension) {
        return Err(Error::ExtensionNotRegistered(extension));
    }

    // Creating the directory for the processed asset that has the same name as the unprocessed asset.
    let processed_asset_path = directories.processed_assets_path().join(&asset_path);
    info!("Creating directory for processed assets: {processed_asset_path:?}");
    fs::create_dir_all(&processed_asset_path)?;

    let unprocessed_asset_path = directories.unprocessed_assets_path().join(&asset_path);

    let senders2 = senders.clone();
    let asset_key2 = asset_key.clone();
    thread_pool.spawn(move || {
        let processors = processors.lock();
        let processor = processors
            .get(&extension)
            // The process function checks if the extension is registered, so this should never happen.
            .expect("processor not found")
            .clone();
        if !unprocessed_asset_path.exists() {
            info!("Asset '{asset_path:?}' was deleted before it could be processed");
            return;
        }
        processor(&asset_key2, &unprocessed_asset_path, &processed_asset_path);

        // Send a Processed event to all observers and remove the channels
        // that are no longer active.
        let mut senders = senders2.lock();
        let mut outdated_channels = Vec::new();
        for (index, sender) in senders.iter().enumerate() {
            if let Err(err) = sender.send(Event::Processed(asset_path.clone())) {
                warn!("Failed to send Processed event for asset {asset_path:?}: \"{err}\". Channel will be removed.");
                outdated_channels.push(index);
            } else {
                info!("Sent Processed event for asset {asset_path:?}");
            }
        }
        for index in outdated_channels.into_iter().rev() {
            senders.remove(index);
        }
    });

    Ok(())
}

/// Iterates through all unprocessed assets and checks whether they are outdated.
fn run_inventory(
    directories: &Directories,
    thread_pool: &ThreadPool,
    processors: &Arc<Mutex<BTreeMap<String, Arc<ProcessFn>>>>,
    senders: &Arc<Mutex<Vec<Sender<Event>>>>,
) -> Result<()> {
    let mut inventory = BTreeMap::new();

    let extensions = processors.lock().keys().cloned().collect::<HashSet<_>>();

    for entry in WalkDir::new(&directories.unprocessed_assets_path) {
        let Ok(entry) = entry else {
                warn!("Failed to read directory entry in WalkDir {:?}: {}", entry, entry.as_ref().unwrap_err());
                continue;
            };

        if entry.path().is_dir() {
            continue;
        }

        // We are only interested in files with registered extensions.
        let extension = if let Ok(extension) = extract_extension_from_path(entry.path()) {
            if !extensions.contains(&extension) {
                continue;
            }
            extension
        } else {
            info!("Failed to extract extension from path: {:?}", entry.path());
            continue;
        };

        // Check if the processed asset exists.
        let processed_asset_path = directories.processed_assets_path.join(entry.path());
        if !processed_asset_path.exists() {
            info!("Asset is going to be processed because it doesn't exist yet: {processed_asset_path:?}");
            inventory.entry(extension).or_insert_with(Vec::new).push(entry.path().to_owned());
            continue;
        }

        // Check if the processed asset is outdated.
        let Some(unprocessed_modified) = modified_system_time(entry.path()) else {
            info!("Failed to read metadata for unprocessed file: {:?}", entry.path());
            continue;
        };
        let Some(processed_modified) = modified_system_time(&processed_asset_path) else {
            info!("Failed to read metadata for processed file: {:?}", processed_asset_path);
            continue;
        };
        if processed_modified < unprocessed_modified {
            info!("Asset is going to be processed because it is outdated: {processed_asset_path:?}");
            inventory.entry(extension).or_insert_with(Vec::new).push(entry.path().to_owned());
        }
    }

    for (_, asset_paths) in inventory {
        for asset_path in asset_paths {
            process(&AssetKey::new(asset_path), directories, &thread_pool, &processors, &senders)?;
        }
    }

    Ok(())
}

/// Builder for creating a processed asset.
pub struct AssetBuilder {
    asset_key: AssetKey,
    unprocessed_asset_path: PathBuf,
    processed_asset_path: PathBuf,
    relative_content_file_path: Option<PathBuf>,
}

impl AssetBuilder {
    /// Creates a new [`AssetBuilder`].
    fn new(asset_key: impl Into<AssetKey>, unprocessed_asset_path: impl Into<PathBuf>, processed_asset_path: impl Into<PathBuf>) -> Self {
        Self {
            asset_key: asset_key.into(),
            unprocessed_asset_path: unprocessed_asset_path.into(),
            processed_asset_path: processed_asset_path.into(),
            relative_content_file_path: None,
        }
    }

    /// Returns the [`AssetKey`] of the asset.
    pub fn asset_key(&self) -> &AssetKey {
        &self.asset_key
    }

    /// Path to the file that is the unprocessed asset.
    pub fn unprocessed_asset_path(&self) -> &Path {
        &self.unprocessed_asset_path
    }

    /// Path to the directory where the processed asset is located.
    ///
    /// This is that directory that is named after the unprocessed asset and is located at
    /// the same relative path in the processed assets directory.
    pub fn processed_asset_path(&self) -> &Path {
        &self.processed_asset_path
    }

    /// Sets the content file of the asset. This is the file that contains the actual data that will be loaded at runtime.
    pub fn with_file(&mut self, relative_file_path: impl Into<PathBuf>) -> &mut Self {
        self.relative_content_file_path = Some(relative_file_path.into());
        self
    }

    /// Builds the asset by creating the asset meta file.
    fn build(self) -> io::Result<()> {
        let content_file_path = self.relative_content_file_path.expect("content file path not set");
        let meta_file_path = self.processed_asset_path.join(ASSET_META_FILE_NAME);
        let meta_file_content = format!("file: {}", content_file_path.display());
        fs::write(meta_file_path, meta_file_content)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::Duration,
    };

    use jeriya_test::setup_logger;
    use tempdir::TempDir;

    use crate::{processor::Event, AssetProcessor, Directories, ProcessConfiguration};

    /// Creates an unprocessed asset with the given content.
    fn create_unprocessed_asset(root: &Path, content: &str) -> PathBuf {
        const ASSET_PATH: &str = "test.txt";
        let asset_path = root.join(ASSET_PATH);
        fs::write(&asset_path, content).unwrap();
        ASSET_PATH.into()
    }

    fn setup_dummy_txt_process_configuration(asset_processor: &mut AssetProcessor) {
        asset_processor
            .register(ProcessConfiguration {
                extension: "txt".to_owned(),
                processor: Box::new(|asset_builder| {
                    let content = fs::read_to_string(asset_builder.unprocessed_asset_path()).unwrap();
                    let processed_content = content.replace("World", "Universe");
                    const CONTENT_FILE: &'static str = "test.bin";
                    let content_file_path = asset_builder.processed_asset_path.join(CONTENT_FILE);
                    fs::write(&content_file_path, processed_content).unwrap();
                    asset_builder.with_file("test.bin");
                    Ok(())
                }),
            })
            .unwrap();
    }

    #[test]
    fn smoke1() {
        setup_logger();
        let root = TempDir::new("root").unwrap();
        let directories =
            Directories::create_all_dir(root.path().to_owned().join("unprocessed"), root.path().to_owned().join("processed")).unwrap();

        // Setup the AssetProcessor.
        let mut asset_processor = AssetProcessor::new(&directories, 4).unwrap();
        setup_dummy_txt_process_configuration(&mut asset_processor);
        let observer_channel = asset_processor.observe();
        asset_processor.set_active(true);

        // Create a sample asset to be processed.
        let asset_path = create_unprocessed_asset(&directories.unprocessed_assets_path(), "Hello World!");

        // Expect the Processed event from the create operation.
        let event = observer_channel.recv_timeout(Duration::from_millis(1500)).unwrap();
        assert_eq!(event, Event::Processed(asset_path.clone()));

        // Expect the Processed event from the modify operation.
        let event = observer_channel.recv_timeout(Duration::from_millis(1500)).unwrap();
        assert_eq!(event, Event::Processed(asset_path.clone()));

        let asset_folder = directories.processed_assets_path().join(&asset_path);
        assert!(asset_folder.join("test.bin").exists());
        let asset_meta_file_path = asset_folder.join("asset.yaml");
        assert!(asset_meta_file_path.exists());
        let meta_file_content = fs::read_to_string(&asset_meta_file_path).unwrap();
        assert_eq!(meta_file_content, "file: test.bin");
    }
}
