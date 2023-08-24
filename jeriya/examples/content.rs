use std::{fs, io};

use jeriya_content::{AssetImporter, AssetProcessor, Directories, Error, FileSystem, ImportConfiguration};
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

    let directories = Directories::create_all_dir("assets/unprocessed_assets", "assets/processed_assets").unwrap();
    let _asset_processor = AssetProcessor::new(&directories, 4).unwrap().register(
        "txt",
        Box::new(|asset_builder| {
            // Just move the text without any processing
            let content = fs::read_to_string(asset_builder.unprocessed_asset_path()).unwrap();
            fs::write(asset_builder.processed_asset_path().join("test.bin"), content).unwrap();
            Ok(())
        }),
    );

    let import_source = FileSystem::new(directories.processed_assets_path()).unwrap();
    let mut asset_importer = AssetImporter::new(import_source, 4).unwrap();

    asset_importer
        .register(ImportConfiguration {
            extension: "txt".to_owned(),
            importer: Box::new(|data| {
                std::str::from_utf8(data)
                    .map_err(|err| Error::Other(Box::new(err)))
                    .map(|s| s.to_owned())
            }),
        })
        .unwrap();
    let mut receiver = asset_importer.receiver::<String>().unwrap();

    loop {
        match receiver.recv().unwrap().as_ref() {
            Ok(asset) => println!("{:?}", asset.value()),
            Err(err) => eprintln!("{err}"),
        }
    }
}
