use std::{fs, io};

use jeriya_content::{AssetImporter, AssetProcessor, Directories, Error, FileSystem, ImportConfiguration, ProcessConfiguration};
use jeriya_shared::log;

fn main() -> io::Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                jeriya_shared::chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Trace)
        .chain(io::stdout())
        .apply()
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

    let unprocessed_assets_path = "assets/unprocessed_assets";
    let processed_assets_path = "assets/processed_assets";
    fs::create_dir_all(unprocessed_assets_path).unwrap();
    fs::create_dir_all(processed_assets_path).unwrap();

    let directories = Directories::create_all_dir(unprocessed_assets_path, processed_assets_path).unwrap();
    let mut asset_processor = AssetProcessor::new(&directories, 4).unwrap();
    asset_processor
        .register(ProcessConfiguration {
            extension: "txt".to_owned(),
            processor: Box::new(|asset_builder| {
                // Just move the text without any processing
                let content = fs::read_to_string(asset_builder.unprocessed_asset_path()).unwrap();
                fs::write(asset_builder.processed_asset_path().join("test.bin"), content).unwrap();
                Ok(())
            }),
        })
        .unwrap();

    let import_source = FileSystem::new(processed_assets_path).unwrap();
    let mut asset_importer = AssetImporter::new(import_source, 4).unwrap();

    let receiver = asset_importer
        .register(ImportConfiguration {
            extension: "txt".to_owned(),
            importer: Box::new(|data| {
                std::str::from_utf8(data)
                    .map_err(|err| Error::Other(Box::new(err)))
                    .map(|s| s.to_owned())
            }),
        })
        .unwrap();

    loop {
        match receiver.recv().unwrap() {
            Ok(asset) => println!("{:?}", asset.value()),
            Err(err) => eprintln!("{err}"),
        }
    }
}
