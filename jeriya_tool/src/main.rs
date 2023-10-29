use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use color_eyre as ey;
use ey::eyre::Context;
use jeriya_content::{model::Model, point_cloud::PointCloud};

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
    let command_line_arguments = CommandLineArguments::parse();
    match &command_line_arguments {
        CommandLineArguments::Convert(convert) => {
            let model = Model::import(&convert.source_filepath).wrap_err("Failed to import model")?;
            let point_cloud = PointCloud::sample_from_model(&model, 2000.0);
            point_cloud
                .to_obj(&convert.destination_filepath)
                .wrap_err("Failed to write point cloud to OBJ")?;
        }
    }
    Ok(())
}
