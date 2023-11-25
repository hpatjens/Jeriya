use std::{fs::File, io, path::PathBuf};

use clap::Parser;
use color_eyre as ey;
use ey::eyre::Context;
use jeriya_content::{
    model::Model,
    point_cloud::{self, PointCloud},
};
use jeriya_shared::log::{self, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum CommandLineArguments {
    Convert(Convert),
}

#[derive(Parser, Debug)]
struct Convert {
    /// Type of the convertion to perform
    #[clap(subcommand)]
    convert_type: ConvertType,

    /// Source file
    #[arg(short, long)]
    source_filepath: PathBuf,

    /// Destination file
    #[arg(short, long)]
    destination_filepath: PathBuf,
}

#[derive(Parser, Debug, Clone, Copy)]
enum ConvertType {
    GltfToPointCloud {
        /// Number of points that will be distributed per square unit
        #[clap(short, long, default_value = "1")]
        points_per_square_unit: f32,
    },
    PointCloudToObj {
        /// Size of the points in the point cloud
        #[clap(short, long, default_value = "0.01")]
        point_size: f32,
    },
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
        CommandLineArguments::Convert(convert) => match convert.convert_type {
            ConvertType::GltfToPointCloud {
                points_per_square_unit: point_per_square_unit,
            } => {
                info!("Importing model: {:?}", convert.source_filepath);
                let model = Model::import(&convert.source_filepath).wrap_err("Failed to import model")?;

                info!("Converting model to point cloud");
                let point_cloud = PointCloud::sample_from_model(&model, point_per_square_unit);

                info!("Serializing point cloud");
                point_cloud
                    .serialize_to_file(&convert.destination_filepath)
                    .wrap_err("Failed to serialize point cloud")?;
            }
            ConvertType::PointCloudToObj { point_size } => {
                info!("Deserializing point cloud");
                let point_cloud =
                    PointCloud::deserialize_from_file(&convert.source_filepath).wrap_err("Failed to deserialize point cloud")?;

                info!("Writing point cloud to OBJ");
                let config = point_cloud::ObjWriteConfig {
                    source: point_cloud::ObjWriteSource::SimplePointCloud,
                    point_size,
                };
                point_cloud
                    .to_obj_file(&config, &convert.destination_filepath)
                    .wrap_err("Failed to write point cloud to OBJ")?;
            }
        },
    }
    Ok(())
}
