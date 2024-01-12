use std::{io, path::PathBuf};

use clap::Parser;
use color_eyre as ey;
use ey::eyre::Context;
use jeriya_content::{
    model::Model,
    point_cloud::{
        clustered_point_cloud::{ClusteredPointCloud, ObjClusterWriteConfig},
        simple_point_cloud::SimplePointCloud,
    },
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

        /// Depth of the clusters that will be writtens
        #[clap(short, long, default_value = "0")]
        depth: usize,
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

                info!("Converting model to simple point cloud");
                let simple_point_cloud = SimplePointCloud::sample_from_model(&model, point_per_square_unit);

                info!("Clustering point cloud");
                let clustered_point_cloud = ClusteredPointCloud::from_simple_point_cloud(&simple_point_cloud);

                info!("Serializing point cloud");
                clustered_point_cloud
                    .serialize_to_file(&convert.destination_filepath)
                    .wrap_err("Failed to serialize point cloud")?;
            }
            ConvertType::PointCloudToObj { point_size, depth } => {
                info!("Deserializing point cloud");
                let clustered_point_cloud =
                    ClusteredPointCloud::deserialize_from_file(&convert.source_filepath).wrap_err("Failed to deserialize point cloud")?;

                info!("Writing point cloud to OBJ");
                clustered_point_cloud
                    .to_obj_file(&ObjClusterWriteConfig::Points { point_size, depth }, &convert.destination_filepath)
                    .wrap_err("Failed to write point cloud to OBJ")?;
            }
        },
    }
    Ok(())
}
