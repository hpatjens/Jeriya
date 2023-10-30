use std::{io, path::PathBuf};

use clap::{Parser, ValueEnum};
use color_eyre as ey;
use ey::eyre::Context;
use jeriya_content::{model::Model, point_cloud::PointCloud};
use jeriya_shared::log::{self, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum CommandLineArguments {
    Convert(Convert),
}

#[derive(Parser, Debug)]
struct Convert {
    /// Type of the target file
    #[clap(value_enum)]
    target_type: TargetType,

    /// Source file
    source_filepath: PathBuf,

    /// Destination file
    destination_filepath: PathBuf,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum TargetType {
    PointCloudObj,
}

fn main() -> ey::Result<()> {
    // Setup logging
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

    let command_line_arguments = CommandLineArguments::parse();
    match &command_line_arguments {
        CommandLineArguments::Convert(convert) => {
            info!("Importing model: {:?}", convert.source_filepath);
            let model = Model::import(&convert.source_filepath).wrap_err("Failed to import model")?;

            info!("Converting model to point cloud");
            let point_cloud = PointCloud::sample_from_model(&model, 0.001);

            info!("Writing point cloud to OBJ");
            point_cloud
                .to_obj(&convert.destination_filepath)
                .wrap_err("Failed to write point cloud to OBJ")?;
        }
    }
    Ok(())
}
