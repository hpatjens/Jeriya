use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use crate::{
    common::{extract_extension_from_path, modified_system_time, AssetKey, Directories, ASSET_META_FILE_NAME},
    Error, Result,
};
use jeriya_shared::{
    crossbeam_channel::{self, Receiver, Sender},
    log::{error, info, trace, warn},
    parking_lot::Mutex,
    pathdiff,
    walkdir::WalkDir,
};
use notify_debouncer_full::{
    notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher},
    DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};

type ProcessFn = dyn Fn(&AssetKey, &Path, &Path) + Send + Sync;

pub type Processor = dyn Fn(&mut AssetBuilder) -> Result<()> + Send + Sync;

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Processed(PathBuf),
}

pub enum Item {
    Wakeup,
    Process(ProcessItem),
}

pub struct ProcessItem {
    asset_key: AssetKey,
    processor: Arc<ProcessFn>,
}

pub struct AssetProcessor {
    directories: Directories,
    wants_drop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    item_sender: Sender<Item>,
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
    /// use jeriya_content::{asset_processor::{AssetProcessor}, common::Directories};
    /// std::fs::create_dir_all("unprocessed").unwrap();
    /// std::fs::create_dir_all("processed").unwrap();
    /// let directories = Directories::create_all_dir("unprocessed", "processed").unwrap();
    /// let asset_processor = AssetProcessor::new(&directories, 4).unwrap();
    /// ```
    pub fn new(directories: &Directories, num_threads: usize) -> crate::Result<Self> {
        let directories = directories.clone();
        directories.check()?;
        info!("Creating AssetProcessor for '{directories:?}'");

        let event_senders = Arc::new(Mutex::new(Vec::new()));
        let processors = Arc::new(Mutex::new(BTreeMap::new()));

        // The [`AssetProcessor`] has to be started manually after the constructor has run so
        // that the user can register processors and receive events for all assets.
        let running = Arc::new(AtomicBool::new(false));

        let wants_drop = Arc::new(AtomicBool::new(false));

        let (item_sender, item_receiver) = crossbeam_channel::unbounded::<Item>();
        for thread_index in 0..num_threads {
            spawn_thread(&wants_drop, &item_receiver, &directories, &event_senders, thread_index)?;
        }

        let running2 = running.clone();
        let sender2 = item_sender.clone();
        let processors2 = processors.clone();
        let directories2 = directories.clone();
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

                    let processors = processors2.clone();

                    // The file watcher returns absolute paths but he whole asset handling is based on
                    // relative paths because it's irrelavant where on the system they are located.
                    let Some(path) = pathdiff::diff_paths(absolute_path, directories2.unprocessed_assets_path()) else {
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
                            if let Err(err) = process(&asset_key, &directories2, &sender2, &processors) {
                                error!("Failed to process file '{asset_key}': {err}");
                            }
                        }
                        EventKind::Modify(_modify_event) => {
                            if let Err(err) = process(&asset_key, &directories2, &sender2, &processors) {
                                error!("Failed to process file '{asset_key}': {err}");
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(err) => {
                let errs = err.into_iter().map(|err| err.to_string()).collect::<Vec<_>>().join(", ");
                error!("Failed to receive events from file watcher: {errs}",);
            }
        };

        // Start the directory watcher.
        let mut watcher = notify_debouncer_full::new_debouncer(Duration::from_millis(1000), None, watch_fn)
            .map_err(|_| Error::FailedToStartDirectoryWatcher(directories.unprocessed_assets_path().to_owned()))?;
        watcher
            .watcher()
            .watch(directories.unprocessed_assets_path(), RecursiveMode::Recursive)
            .map_err(|_| Error::FailedToStartDirectoryWatcher(directories.unprocessed_assets_path().to_owned()))?;

        Ok(Self {
            directories,
            wants_drop,
            running,
            item_sender,
            senders: event_senders,
            processors,
            _watcher: watcher,
        })
    }

    /// Either sets the [`AssetProcessor`] to active or inactive.
    pub fn set_active(&self, active: bool) -> Result<()> {
        self.running.store(active, Ordering::SeqCst);

        if active {
            run_inventory(&self.directories, &self.item_sender, &self.processors)?;
        }

        Ok(())
    }

    /// Registers a [`Processor`] for the given file extension.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jeriya_content::{asset_processor::{AssetProcessor}, common::Directories};
    /// let directories = Directories::create_all_dir("unprocessed", "processed").unwrap();
    /// let asset_processor = AssetProcessor::new(&directories, 4)
    ///     .unwrap()
    ///     .register(
    ///         "txt",
    ///         Box::new(|asset_builder| {
    ///             let content = std::fs::read_to_string(asset_builder.unprocessed_asset_path()).unwrap();
    ///             let processed_content = content.replace("World", "Universe");
    ///             std::fs::write(asset_builder.processed_asset_path().join("test.bin"), processed_content).unwrap();
    ///             Ok(())
    ///         })
    ///     );
    /// ```
    pub fn register(self, extension: impl Into<String>, processor: Box<Processor>) -> Self {
        let extension = extension.into();
        let mut processors = self.processors.lock();
        if processors.contains_key(&extension) {
            panic!("importer for extension '{extension}' already registered");
        }
        processors.insert(
            extension,
            Arc::new(move |asset_key, unprocessed_asset_path, processed_asset_path| {
                info!("Processing file: {asset_key}");
                let mut asset_builder = AssetBuilder::new(asset_key, unprocessed_asset_path, processed_asset_path);
                let process_result = (processor)(&mut asset_builder);
                match process_result {
                    Ok(()) => match asset_builder.build() {
                        Ok(_) => info!("Successfully processed and built file: {asset_key}"),
                        Err(_) => error!("Failed to build the meta file for asset '{asset_key}'"),
                    },
                    Err(err) => error!("Failed to process file '{asset_key}': {err}"),
                }
            }),
        );
        drop(processors);
        self
    }

    /// Returns a channel that can be used to observe [`Event`]s.
    pub fn observe(&mut self) -> Receiver<Event> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        self.senders.lock().push(sender);
        receiver
    }
}

fn spawn_thread(
    wants_drop: &Arc<AtomicBool>,
    item_receiver: &Receiver<Item>,
    directories: &Directories,
    event_senders: &Arc<Mutex<Vec<Sender<Event>>>>,
    thread_index: usize,
) -> Result<()> {
    let wants_drop = wants_drop.clone();
    let item_receiver = item_receiver.clone();
    let directories = directories.clone();
    let event_senders2 = event_senders.clone();
    let thread_name = format!("AssetProcessor thread {}", thread_index);
    let builder = thread::Builder::new().name(thread_name.clone());
    builder
        .spawn(move || {
            info!("Starting AssetProcessor thread '{thread_name}'");
            loop {
                trace!("Waiting for item on AssetProcessor thread '{thread_name}'");
                let Ok(item) = item_receiver.recv() else {
                    error!("AssetProcessor thread '{thread_name}' failed to receive item");
                    break;
                };
                if wants_drop.load(Ordering::SeqCst) {
                    trace!("AssetProcessor wants to be dropped");
                    break;
                }

                let process_item = match item {
                    Item::Wakeup => continue,
                    Item::Process(process_item) => process_item,
                };
                info!("AssetProcessor starting work on item: {}", process_item.asset_key);

                if !directories.unprocessed_assets_path().exists() {
                    info!("Asset '{}' was deleted before it could be processed", process_item.asset_key);
                    return;
                }
                (process_item.processor)(
                    &process_item.asset_key,
                    &directories.unprocessed_assets_path().join(process_item.asset_key.as_path()),
                    &directories.processed_assets_path().join(process_item.asset_key.as_path()),
                );

                // Send a Processed event to all observers and remove the channels
                // that are no longer active.
                let mut senders = event_senders2.lock();
                let mut outdated_channels = Vec::new();
                for (index, sender) in senders.iter().enumerate() {
                    let asset_path = process_item.asset_key.as_path();
                    if let Err(err) = sender.send(Event::Processed(asset_path.to_owned())) {
                        warn!("Failed to send Processed event for asset {asset_path:?}: \"{err}\". Channel will be removed.");
                        outdated_channels.push(index);
                    } else {
                        info!("Sent Processed event for asset {asset_path:?}");
                    }
                }
                for index in outdated_channels.into_iter().rev() {
                    senders.remove(index);
                }
            }
            info!("AssetProcessor thread '{thread_name}' will stop now");
        })
        .map_err(|_| Error::FailedToStartThreadPool)?;
    Ok(())
}

impl Drop for AssetProcessor {
    fn drop(&mut self) {
        self.wants_drop.store(true, Ordering::SeqCst);
        if let Err(err) = self.item_sender.send(Item::Wakeup) {
            error!("Failed to send wakeup item to AssetProcessor in drop implementation: {err}");
        }
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
    sender: &Sender<Item>,
    processors: &Arc<Mutex<BTreeMap<String, Arc<ProcessFn>>>>,
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

    trace!("Querying the processor for the extension '{extension}'");
    let Some(processor) = processors.lock().get(&extension).cloned() else {
        return Err(Error::ExtensionNotRegistered(extension));
    };

    // Creating the directory for the processed asset that has the same name as the unprocessed asset.
    let processed_asset_path = directories.processed_assets_path().join(&asset_path);
    info!("Creating directory for processed assets: {processed_asset_path:?}");
    fs::create_dir_all(&processed_asset_path)?;

    let item = Item::Process(ProcessItem {
        asset_key: asset_key.clone(),
        processor,
    });
    if let Err(err) = sender.send(item) {
        error!("Failed to send item to AssetProcessor thread: {err}");
    }

    Ok(())
}

/// Iterates through all unprocessed assets and checks whether they are outdated.
fn run_inventory(
    directories: &Directories,
    sender: &Sender<Item>,
    processors: &Arc<Mutex<BTreeMap<String, Arc<ProcessFn>>>>,
) -> Result<()> {
    let mut inventory = BTreeMap::new();

    let extensions = processors.lock().keys().cloned().collect::<HashSet<_>>();

    let path = directories.unprocessed_assets_path();
    info!("Running inventory in path: {path:?}");

    for entry in WalkDir::new(path) {
        let Ok(entry) = entry else {
            warn!(
                "Failed to read directory entry in WalkDir {:?}: {}",
                entry,
                entry.as_ref().unwrap_err()
            );
            continue;
        };

        if entry.path().is_dir() {
            continue;
        }

        let Some(relative_path) = pathdiff::diff_paths(entry.path(), path) else {
            warn!("Failed to get relative path of '{}' relative to '{path:?}'", entry.path().display());
            continue;
        };
        let asset_key = AssetKey::new(relative_path);

        // We are only interested in files with registered extensions.
        let extension = if let Ok(extension) = extract_extension_from_path(asset_key.as_path()) {
            if !extensions.contains(&extension) {
                continue;
            }
            extension
        } else {
            info!("Failed to extract extension from path: {:?}", asset_key.as_path());
            continue;
        };

        // Check if the processed asset exists.
        let processed_asset_path = directories.processed_assets_path().join(asset_key.as_path());
        if !processed_asset_path.exists() {
            info!("Asset is going to be processed because it doesn't exist yet: {processed_asset_path:?}");
            inventory
                .entry(extension)
                .or_insert_with(Vec::new)
                .push(asset_key.as_path().to_owned());
            continue;
        }

        // Check if the processed asset is outdated.
        let Some(unprocessed_modified) = modified_system_time(asset_key.as_path()) else {
            continue;
        };
        let Some(processed_modified) = modified_system_time(&processed_asset_path) else {
            continue;
        };
        if processed_modified < unprocessed_modified {
            info!("Asset is going to be processed because it is outdated: {processed_asset_path:?}");
            inventory
                .entry(extension)
                .or_insert_with(Vec::new)
                .push(asset_key.as_path().to_owned());
            continue;
        }

        trace!("Asset doesn't need to be processed: {}", asset_key.as_path().display());
    }

    trace!("Found {} assets to process", inventory.len());
    for (_, asset_paths) in inventory {
        for asset_path in asset_paths {
            process(&AssetKey::new(asset_path), directories, sender, processors)?;
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
    pub(crate) fn new(
        asset_key: impl Into<AssetKey>,
        unprocessed_asset_path: impl Into<PathBuf>,
        processed_asset_path: impl Into<PathBuf>,
    ) -> Self {
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

    use crate::{asset_processor::Event, common::Directories};

    use super::AssetProcessor;

    /// Creates an unprocessed asset with the given content.
    fn create_unprocessed_asset(root: &Path, content: &str) -> PathBuf {
        const ASSET_PATH: &str = "test.txt";
        let asset_path = root.join(ASSET_PATH);
        fs::write(&asset_path, content).unwrap();
        ASSET_PATH.into()
    }

    fn setup_dummy_txt_process_configuration(asset_processor: AssetProcessor) -> AssetProcessor {
        asset_processor.register(
            "txt",
            Box::new(|asset_builder| {
                let content = fs::read_to_string(asset_builder.unprocessed_asset_path()).unwrap();
                let processed_content = content.replace("World", "Universe");
                const CONTENT_FILE: &'static str = "test.bin";
                let content_file_path = asset_builder.processed_asset_path.join(CONTENT_FILE);
                fs::write(&content_file_path, processed_content).unwrap();
                asset_builder.with_file("test.bin");
                Ok(())
            }),
        )
    }

    #[test]
    fn smoke1() {
        setup_logger();
        let root = TempDir::new("root").unwrap();
        let directories =
            Directories::create_all_dir(root.path().to_owned().join("unprocessed"), root.path().to_owned().join("processed")).unwrap();

        // Setup the AssetProcessor.
        let mut asset_processor = setup_dummy_txt_process_configuration(AssetProcessor::new(&directories, 4).unwrap());
        let observer_channel = asset_processor.observe();
        asset_processor.set_active(true).unwrap();

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
