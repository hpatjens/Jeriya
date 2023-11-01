use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use gltf::{
    buffer::Data,
    mesh::{util::ReadIndices, Mode},
};
use jeriya_shared::{
    log::{info, trace},
    nalgebra::Vector3,
    thiserror, ByteColor4,
};
use serde::{Deserialize, Serialize};

use crate::AssetBuilder;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to load model '{path}': {error_message}")]
    FailedLoading { path: PathBuf, error_message: String },
    #[error("Model has no vertex positions")]
    NoVertexPositions,
    #[error("Model has no vertex normals")]
    NoVertexNormals,
}

impl From<Error> for crate::Error {
    fn from(value: Error) -> Self {
        crate::Error::Other(Box::new(value))
    }
}

/// Determines how the OBJ file is generated.
pub enum ObjWriteConfig {
    FromSimpleMesh,
    FromMeshlets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texture {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub base_color_texture_index: Option<usize>,
    pub base_color_color: ByteColor4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub meshes: Vec<Mesh>,
    pub textures: Vec<Texture>,
    pub materials: Vec<Material>,
}

impl Model {
    /// Import model from a glTF file.
    pub fn import(path: impl AsRef<Path>) -> crate::Result<Model> {
        let (document, buffers, images) = gltf::import(&path).map_err(|err| Error::FailedLoading {
            path: path.as_ref().to_owned(),
            error_message: err.to_string(),
        })?;

        let textures = images
            .iter()
            .map(|image| {
                let data = image.pixels.to_owned();
                let width = image.width;
                let height = image.height;
                Texture { data, width, height }
            })
            .collect::<Vec<_>>();

        let materials = document
            .materials()
            .map(|material| {
                let base_color_color = ByteColor4::from(material.pbr_metallic_roughness().base_color_factor());
                let base_color_texture_index = material
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|texture| texture.texture().index());
                Material {
                    name: material.name().unwrap_or("unknown").to_owned(),
                    base_color_texture_index,
                    base_color_color,
                }
            })
            .collect::<Vec<_>>();

        let model_name = path.as_ref().to_str().unwrap_or("unknown");
        let meshes = document
            .meshes()
            .map(|mesh| build_mesh(model_name, &mesh, &buffers))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Model {
            name: model_name.to_owned(),
            meshes,
            textures,
            materials,
        })
    }

    /// Writes the model to an OBJ file. The MTL file must be written to the same directory as the OBJ file. `mtl_reference_name` must be the filename of the MTL file.
    pub fn to_obj(
        &self,
        config: ObjWriteConfig,
        obj_writer: impl Write,
        mtl_writer: impl Write,
        mtl_reference_name: &str,
    ) -> crate::Result<()> {
        match &config {
            ObjWriteConfig::FromSimpleMesh => self.to_obj_from_simple_mesh(obj_writer, mtl_writer, mtl_reference_name),
            ObjWriteConfig::FromMeshlets => self.to_obj_from_meshlets(obj_writer, mtl_writer, mtl_reference_name),
        }
    }

    fn to_obj_from_simple_mesh(
        &self,
        mut obj_writer: impl Write,
        mut mtl_writer: impl Write,
        mtl_reference_name: &str,
    ) -> crate::Result<()> {
        // Write OBJ file
        let mut written_vertices = 0u32;
        writeln!(obj_writer, "mtllib {}", mtl_reference_name)?;
        for (mesh_index, mesh) in self.meshes.iter().enumerate() {
            writeln!(obj_writer, "o mesh_{}", mesh_index)?;
            writeln!(obj_writer, "usemtl mesh_{}", mesh_index)?;
            for vertex in &mesh.simple_mesh.vertex_positions {
                writeln!(obj_writer, "v {} {} {}", vertex.x, vertex.y, vertex.z)?;
            }
            for chunk in mesh.simple_mesh.indices.rchunks(3) {
                jeriya_shared::assert!(chunk.len() == 3, "Expected indices to be a multiple of 3");
                let base = written_vertices + 1;
                let i0 = base + chunk[0];
                let i1 = base + chunk[1];
                let i2 = base + chunk[2];
                writeln!(obj_writer, "f {i0} {i1} {i2}")?;
            }
            written_vertices += mesh.simple_mesh.vertex_positions.len() as u32;
        }

        // Write MTL file
        for (mesh_index, _mesh) in self.meshes.iter().enumerate() {
            writeln!(mtl_writer, "newmtl mesh_{}", mesh_index)?;
            let color = jeriya_shared::pseudo_random_color(mesh_index);
            writeln!(mtl_writer, "Ka {} {} {}", color[0], color[1], color[2])?;
            writeln!(mtl_writer, "Kd {} {} {}", color[0], color[1], color[2])?;
            writeln!(mtl_writer, "Ks {} {} {}", color[0], color[1], color[2])?;
            writeln!(mtl_writer, "Ns 10.0")?;
        }

        Ok(())
    }

    fn to_obj_from_meshlets(&self, mut obj_writer: impl Write, mut mtl_writer: impl Write, mtl_reference_name: &str) -> crate::Result<()> {
        // Write OBJ file
        let mut written_vertices = 0u32;
        writeln!(obj_writer, "mtllib {}", mtl_reference_name)?;
        for (mesh_index, mesh) in self.meshes.iter().enumerate() {
            for vertex in &mesh.simple_mesh.vertex_positions {
                writeln!(obj_writer, "v {} {} {}", vertex.x, vertex.y, vertex.z)?;
            }
            for (meshlet_index, meshlet) in mesh.meshlets.iter().enumerate() {
                writeln!(obj_writer, "o mesh_{mesh_index}_meshlet_{meshlet_index}")?;
                writeln!(obj_writer, "usemtl mesh_{mesh_index}_meshlet_{meshlet_index}")?;
                for chunk in &meshlet.local_indices {
                    let base = written_vertices + 1;
                    let i0 = base + meshlet.global_indices[chunk[0] as usize];
                    let i1 = base + meshlet.global_indices[chunk[1] as usize];
                    let i2 = base + meshlet.global_indices[chunk[2] as usize];
                    writeln!(obj_writer, "f {i0} {i1} {i2}")?;
                }
            }
            written_vertices += mesh.simple_mesh.vertex_positions.len() as u32;
        }

        // Write MTL file
        for (mesh_index, mesh) in self.meshes.iter().enumerate() {
            for (meshlet_index, _meshlet) in mesh.meshlets.iter().enumerate() {
                writeln!(mtl_writer, "newmtl mesh_{mesh_index}_meshlet_{meshlet_index}")?;
                let color = jeriya_shared::pseudo_random_color(mesh_index * meshlet_index);
                writeln!(mtl_writer, "Ka {} {} {}", color[0], color[1], color[2])?;
                writeln!(mtl_writer, "Kd {} {} {}", color[0], color[1], color[2])?;
                writeln!(mtl_writer, "Ks {} {} {}", color[0], color[1], color[2])?;
                writeln!(mtl_writer, "Ns 10.0")?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub simple_mesh: SimpleMesh,
    pub meshlets: Vec<Meshlet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMesh {
    pub vertex_positions: Vec<Vector3<f32>>,
    pub vertex_normals: Vec<Vector3<f32>>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Meshlet {
    /// Indices into the vertex buffer of the mesh. This contains a maximum of 64 vertices.
    pub global_indices: Vec<u32>,
    /// Indices into the `global_indices` buffer. This contains a maximum of 126 triangles.
    pub local_indices: Vec<[u8; 3]>,
}

impl Meshlet {
    pub const MAX_VERTICES: usize = 64;
    pub const MAX_TRIANGLES: usize = 126;
}

/// Function for the [`AssetProcessor`]
pub fn process_model(asset_builder: &mut AssetBuilder) -> crate::Result<()> {
    let path = asset_builder.unprocessed_asset_path().to_owned();
    let model = Model::import(path)?;
    let file_name = "model.bin";
    let file = File::create(asset_builder.processed_asset_path().join(file_name))?;
    bincode::serialize_into(file, &model).map_err(|err| crate::Error::FailedSerialization(err))?;
    asset_builder.with_file(file_name);
    Ok(())
}

fn build_simple_mesh(mesh: &gltf::Mesh, buffers: &[Data]) -> crate::Result<SimpleMesh> {
    let mut used_vertex_positions = BTreeMap::new();
    let mut used_vertex_normals = BTreeMap::new();
    let mut old_indices = Vec::new();

    for primitive in mesh.primitives() {
        trace!("Primitive mode: {:?}", primitive.mode());
        assert_eq!(primitive.mode(), Mode::Triangles, "Currently only triangles are supported");
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        let temp_vertex_positions = reader.read_positions().ok_or(Error::NoVertexPositions)?.collect::<Vec<_>>();
        let temp_vertex_normals = reader.read_normals().ok_or(Error::NoVertexNormals)?.collect::<Vec<_>>();
        if let Some(indices) = reader.read_indices() {
            match &indices {
                ReadIndices::U8(iter) => {
                    for index in iter.clone() {
                        old_indices.push(index as u32);
                        used_vertex_positions
                            .entry(index as u32)
                            .or_insert(temp_vertex_positions[index as usize]);
                        used_vertex_normals
                            .entry(index as u32)
                            .or_insert(temp_vertex_normals[index as usize]);
                    }
                }
                ReadIndices::U16(iter) => {
                    for index in iter.clone() {
                        old_indices.push(index as u32);
                        used_vertex_positions
                            .entry(index as u32)
                            .or_insert(temp_vertex_positions[index as usize]);
                        used_vertex_normals
                            .entry(index as u32)
                            .or_insert(temp_vertex_normals[index as usize]);
                    }
                }
                ReadIndices::U32(iter) => {
                    for index in iter.clone() {
                        old_indices.push(index);
                        used_vertex_positions.entry(index).or_insert(temp_vertex_positions[index as usize]);
                        used_vertex_normals.entry(index).or_insert(temp_vertex_normals[index as usize]);
                    }
                }
            }
        }
    }

    let mut vertex_positions = Vec::new();
    let mut index_mapping = BTreeMap::new();
    for (new_index, (old_index, vertex)) in used_vertex_positions.into_iter().enumerate() {
        vertex_positions.push(Vector3::new(vertex[0], vertex[1], vertex[2]));
        index_mapping.insert(old_index, new_index as u32);
    }

    let mut vertex_normals = Vec::new();
    for (_, vertex) in used_vertex_normals.into_iter() {
        vertex_normals.push(Vector3::new(vertex[0], vertex[1], vertex[2]));
    }

    let indices = old_indices
        .into_iter()
        .map(|old_index| index_mapping[&old_index])
        .collect::<Vec<_>>();
    let indices = meshopt::optimize::optimize_vertex_cache(&indices, vertex_positions.len());

    Ok(SimpleMesh {
        vertex_positions,
        vertex_normals,
        indices,
    })
}

fn build_meshlets(simple_mesh: &SimpleMesh) -> crate::Result<Vec<Meshlet>> {
    let meshlets = meshopt::clusterize::build_meshlets(&simple_mesh.indices, simple_mesh.vertex_positions.len(), 64, 126);
    let meshlets = meshlets
        .into_iter()
        .map(|meshlet| {
            info!("Meshlet: {:?}", (meshlet.vertex_count, meshlet.triangle_count));
            Meshlet {
                global_indices: meshlet.vertices.into_iter().take(meshlet.vertex_count as usize).collect(),
                local_indices: meshlet.indices.into_iter().take(meshlet.triangle_count as usize).collect(),
            }
        })
        .collect::<Vec<_>>();
    Ok(meshlets)
}

fn build_mesh(model_name: &str, mesh: &gltf::Mesh, buffers: &[Data]) -> crate::Result<Mesh> {
    let name = mesh.name().unwrap_or("unknown");
    trace!("Processing mesh '{name}' in model '{model_name}'");

    let simple_mesh = build_simple_mesh(mesh, buffers)?;
    let meshlets = build_meshlets(&simple_mesh)?;

    let mesh = Mesh { simple_mesh, meshlets };

    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::BufWriter};

    use jeriya_test::setup_logger;

    use super::*;

    const TEST_RESULTS_DIR: &str = "test_results";

    struct Contents {
        obj: String,
        mtl: String,
    }

    fn export(src_path: impl AsRef<Path>, dst_name: &str, obj_write_config: ObjWriteConfig) -> Contents {
        let model = Model::import(&src_path).unwrap();
        let mut obj_writer = BufWriter::new(Vec::new());
        let mut mtl_writer = BufWriter::new(Vec::new());

        let mtl_file_path = Path::new(TEST_RESULTS_DIR).join(dst_name).with_extension("mtl");
        let obj_file_path = Path::new(TEST_RESULTS_DIR).join(dst_name).with_extension("obj");
        let mtl_file_name = mtl_file_path.file_name().unwrap().to_str().unwrap();

        model
            .to_obj(obj_write_config, &mut obj_writer, &mut mtl_writer, mtl_file_name)
            .unwrap();

        let obj_content = String::from_utf8(obj_writer.into_inner().unwrap()).unwrap();
        let mtl_content = String::from_utf8(mtl_writer.into_inner().unwrap()).unwrap();

        fs::create_dir_all(TEST_RESULTS_DIR).unwrap();
        fs::write(obj_file_path, &obj_content).unwrap();
        fs::write(mtl_file_path, &mtl_content).unwrap();

        Contents {
            obj: obj_content,
            mtl: mtl_content,
        }
    }

    fn assert_obj_model(contents: &Contents, expected_model_path: impl AsRef<Path>) {
        let expected_obj = fs::read_to_string(&expected_model_path).unwrap().replace("\r\n", "\n");
        let expected_mtl = fs::read_to_string(&expected_model_path.as_ref().with_extension("mtl"))
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(contents.obj, expected_obj);
        assert_eq!(contents.mtl, expected_mtl);
    }

    #[test]
    fn smoke() {
        setup_logger();
        let model = Model::import("../sample_assets/models/rotated_cube.glb").unwrap();
        assert_eq!(model.meshes.len(), 1);
        assert_eq!(model.meshes[0].simple_mesh.vertex_positions.len(), 24);
        assert_eq!(model.meshes[0].simple_mesh.vertex_normals.len(), 24);
        assert_eq!(model.meshes[0].simple_mesh.indices.len(), 36);
        assert_eq!(model.meshes[0].meshlets.len(), 1);

        for meshlet in &model.meshes[0].meshlets {
            assert_eq!(meshlet.global_indices.len(), 24);
            assert_eq!(meshlet.local_indices.len(), 12); // number of triangles
            for triangle in &meshlet.local_indices {
                assert!((triangle[0] as usize) < meshlet.global_indices.len());
                assert!((triangle[1] as usize) < meshlet.global_indices.len());
                assert!((triangle[2] as usize) < meshlet.global_indices.len());
            }
            for vertex in &meshlet.global_indices {
                assert!((*vertex as usize) < model.meshes[0].simple_mesh.vertex_positions.len() as usize);
            }
        }
    }

    #[test]
    fn obj_export_rotated_cube() {
        setup_logger();
        let contents = export(
            "../sample_assets/models/rotated_cube.glb",
            "rotated_cube",
            ObjWriteConfig::FromSimpleMesh,
        );
        assert_obj_model(&contents, "expected_results/rotated_cube.obj");
    }

    #[test]
    fn obj_export_suzanne_simple_mesh() {
        setup_logger();
        let contents = export(
            "../sample_assets/models/suzanne.glb",
            "suzanne_simple_mesh",
            ObjWriteConfig::FromSimpleMesh,
        );
        assert_obj_model(&contents, "expected_results/suzanne_simple_mesh.obj");
    }

    #[test]
    fn obj_export_suzanne_meshlets() {
        setup_logger();
        let contents = export(
            "../sample_assets/models/suzanne.glb",
            "suzanne_meshlets",
            ObjWriteConfig::FromMeshlets,
        );
        assert_obj_model(&contents, "expected_results/suzanne_meshlets.obj");
    }
}
