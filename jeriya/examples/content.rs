use std::io;

use jeriya_content::{AssetImporter, Error, FileSystem, ImportConfiguration};
use jeriya_shared::log::{self};

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

    let import_source = FileSystem::new("assets").unwrap();
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
