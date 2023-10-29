use std::{fs::File, io::Write, path::PathBuf};

use clap::{Parser, ValueEnum};
use color_eyre as ey;
use ey::eyre::Context;
use jeriya_content::model::Model;
use jeriya_shared::random_direction;

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
    PointCloud,
}

fn main() -> ey::Result<()> {
    let command_line_arguments = CommandLineArguments::parse();
    match &command_line_arguments {
        CommandLineArguments::Convert(convert) => {
            handle_convert(convert).wrap_err("Failed to convert")?;
        }
    }
    Ok(())
}

fn handle_convert(convert: &Convert) -> ey::Result<()> {
    let mut file = File::create(&convert.destination_filepath).wrap_err("Failed to create destination file")?;

    let model = Model::import(&convert.source_filepath).wrap_err("Failed to import model")?;
    for (mesh_index, mesh) in model.meshes.iter().enumerate() {
        writeln!(file, "o {}-{}", model.name, mesh_index)?;
        for triangle in mesh.simple_mesh.indices.chunks(3) {
            // Vertices of the current triangle
            let a = mesh.simple_mesh.vertex_positions[triangle[0] as usize];
            let b = mesh.simple_mesh.vertex_positions[triangle[1] as usize];
            let c = mesh.simple_mesh.vertex_positions[triangle[2] as usize];
            let center = (a + b + c) / 3.0;

            // Creating a coordinate system
            let u = random_direction();
            let mut v = random_direction();
            while v == u {
                v = random_direction();
            }
            let n = u.cross(&v).normalize();

            // Creating a triangle
            const K: f32 = 0.01;
            let a = center;
            let b = center + K * u;
            let c = center + K * n;
            match convert.target_type {
                TargetType::PointCloud => {
                    writeln!(file, "v {} {} {}", a.x, a.y, a.z)?;
                    writeln!(file, "v {} {} {}", b.x, b.y, b.z)?;
                    writeln!(file, "v {} {} {}", c.x, c.y, c.z)?;
                }
            }
        }
        for index in 0..mesh.simple_mesh.indices.len() / 3 {
            writeln!(file, "f {} {} {}", 3 * index + 1, 3 * index + 2, 3 * index + 3)?;
        }
    }

    Ok(())
}
