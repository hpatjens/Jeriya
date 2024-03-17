use std::{collections::BTreeMap, path::Path, sync::Arc};

use color_eyre as ey;
use color_eyre::eyre::Context;
use ey::eyre::eyre;
use jeriya::Renderer;
use jeriya_backend::{
    elements::{
        element_group::ElementGroup,
        helper::{rigid_mesh_collection::RigidMeshCollection, rigid_mesh_instance_collection::RigidMeshInstanceCollection},
        point_cloud::PointCloud,
    },
    instances::{instance_group::InstanceGroup, point_cloud_instance::PointCloudInstance},
    resources::{point_cloud_attributes::PointCloudAttributes, resource_group::ResourceGroup},
    transactions::Transaction,
    Backend,
};
use jeriya_content::{model::ModelAsset, point_cloud::clustered_point_cloud::ClusteredPointCloudAsset};
use jeriya_shared::{
    debug_info,
    log::{info, trace},
    nalgebra::{self, Scale3, Translation3},
    parking_lot::Mutex,
};

use crate::{scene_description::SceneDescription, FileType};

pub fn load_scene<B: Backend>(
    path: impl AsRef<Path>,
    file_type: Option<FileType>,
    scale: f32,
    renderer: &Arc<Renderer<B>>,
    resource_group: &Arc<Mutex<ResourceGroup>>,
    element_group: &Arc<Mutex<ElementGroup>>,
    instance_group: &Arc<Mutex<InstanceGroup>>,
) -> ey::Result<()> {
    if let Some(file_type) = file_type {
        match file_type {
            FileType::Scene => load_scene_description(&path, resource_group, element_group, instance_group, renderer),
            FileType::Model => load_model(&path, resource_group, element_group, instance_group, renderer),
            FileType::PointCloud => load_point_cloud(path, resource_group, element_group, instance_group, renderer, scale),
        }
    } else {
        match path.as_ref().extension().and_then(|ext| ext.to_str()) {
            Some("yaml") => load_scene_description(&path, resource_group, element_group, instance_group, renderer),
            Some("glb" | "gltf") => load_model(&path, resource_group, element_group, instance_group, renderer),
            Some(extension) => Err(eyre!("Unknown file extension: {:?}", extension)),
            None => Err(eyre!("Failed to determine file type from path: {:?}", path.as_ref())),
        }
    }
}

fn load_point_cloud<B: Backend>(
    path: impl AsRef<Path>,
    resource_group: &Arc<Mutex<ResourceGroup>>,
    element_group: &Arc<Mutex<ElementGroup>>,
    instance_group: &Arc<Mutex<InstanceGroup>>,
    renderer: &Arc<Renderer<B>>,
    scale: f32,
) -> ey::Result<()> {
    let clustered_point_cloud = ClusteredPointCloudAsset::deserialize_from_file(&path)
        .wrap_err("Failed to deserialize PointCloud")
        .expect("Failed to deserialize PointCloud");
    info!("PointCloud to view: {clustered_point_cloud:?}");

    let mut resource_group = resource_group.lock();
    let mut element_group = element_group.lock();
    let mut instance_group = instance_group.lock();

    // Create PointCloudAttributes
    let point_cloud_attributes_builder = PointCloudAttributes::builder()
        .with_debug_info(debug_info!("my_point_cloud_attributes"))
        .with_pages(clustered_point_cloud.pages().to_vec())
        .with_root_cluster_index(clustered_point_cloud.root_cluster_index().clone());
    let point_cloud_attributes = resource_group
        .point_cloud_attributes()
        .insert_with(point_cloud_attributes_builder)
        .expect("Failed to insert PointCloudAttributes");

    let mut transaction = Transaction::record(renderer);

    // Create PointCloud
    let point_cloud_builder = PointCloud::builder()
        .with_point_cloud_attributes(point_cloud_attributes)
        .with_debug_info(debug_info!("my_point_cloud"));
    let point_cloud = element_group
        .point_clouds()
        .mutate_via(&mut transaction)
        .insert_with(point_cloud_builder)
        .expect("Failed to insert PointCloud");

    // Create PointCloudInstance
    let point_cloud_instance_builder = PointCloudInstance::builder()
        .with_point_cloud(element_group.point_clouds().get(&point_cloud).unwrap())
        .with_transform(Scale3::new(scale, scale, scale).into())
        .with_debug_info(debug_info!("my_point_cloud_instance"));
    let _point_cloud_instance = instance_group
        .point_cloud_instances()
        .mutate_via(&mut transaction)
        .insert_with(point_cloud_instance_builder)
        .expect("Failed to insert PointCloudInstance");

    transaction.finish();

    Ok(())
}

fn load_model<B: Backend>(
    path: &impl AsRef<Path>,
    resource_group: &Arc<Mutex<ResourceGroup>>,
    element_group: &Arc<Mutex<ElementGroup>>,
    instance_group: &Arc<Mutex<InstanceGroup>>,
    renderer: &Arc<Renderer<B>>,
) -> ey::Result<()> {
    let main_model = ModelAsset::import(path)
        .wrap_err("Failed to import model")
        .expect("Failed to import model");

    let mut resource_group = resource_group.lock();
    let mut element_group = element_group.lock();
    let mut instance_group = instance_group.lock();

    let mut transaction = Transaction::record(renderer);

    // Create a RigidMesh from model
    //
    // A RigidMeshCollection can be used to create multiple RigidMeshes from a single Model. To display
    // the RigidMeshes in the scene, RigidMeshInstances must be created that reference the RigidMeshes.
    // A RigidMeshInstanceCollection can be used for that.
    let rigid_mesh_collection = RigidMeshCollection::from_model(&main_model, &mut resource_group, &mut element_group, &mut transaction)
        .expect("Failed to create RigidMeshCollection");
    let _rigid_mesh_instance_collection = RigidMeshInstanceCollection::from_rigid_mesh_collection(
        &rigid_mesh_collection,
        element_group.rigid_meshes(),
        &mut instance_group,
        &mut transaction,
        &nalgebra::convert(Translation3::new(0.0, 0.0, 0.0)),
    )
    .expect("Failed to create RigidMeshInstanceCollection");

    transaction.finish();

    Ok(())
}

fn load_scene_description<B: Backend>(
    path: &impl AsRef<Path>,
    resource_group: &Arc<Mutex<ResourceGroup>>,
    element_group: &Arc<Mutex<ElementGroup>>,
    instance_group: &Arc<Mutex<InstanceGroup>>,
    renderer: &Arc<Renderer<B>>,
) -> ey::Result<()> {
    let scene_base_path = Path::new(path.as_ref()).parent();
    let scene = SceneDescription::import(path.as_ref()).wrap_err("Failed to import SceneDescription")?;
    info!("Scene to view: {scene:?}");

    let mut resource_group = resource_group.lock();
    let mut element_group = element_group.lock();
    let mut instance_group = instance_group.lock();

    // Find the unique model paths
    //
    // Every instance entry in the SceneDescription has the path directly associated with it. It makes sense
    // to find all unique paths first and load the RigidMeshes from those instead of loading the same RigidMesh
    // multiple times.
    let rigid_meshes = scene
        .rigid_mesh_instances
        .iter()
        .map(|instance| {
            // The paths in the SceneDescription are relative to the SceneDescription file
            let absolute_path = if instance.path.is_absolute() {
                instance.path.clone()
            } else {
                // When a path doesn't have a parent path, we assume that we can treat is as an absolute path
                if let Some(scene_base_path) = scene_base_path {
                    scene_base_path.join(&instance.path)
                } else {
                    instance.path.clone()
                }
            };
            (instance.path.clone(), absolute_path)
        })
        .collect::<BTreeMap<_, _>>();
    trace!("RigidMesh count in Scene: {}", rigid_meshes.len());

    // Create RigidMeshCollections from the unique paths
    let rigid_mesh_collections = rigid_meshes
        .iter()
        .map(|(path, absolute_path)| {
            let model = ModelAsset::import(absolute_path).unwrap();
            let rigid_mesh_collection =
                RigidMeshCollection::from_model(&model, &mut resource_group, &mut element_group, &mut Transaction::record(renderer))
                    .unwrap();
            (path, rigid_mesh_collection)
        })
        .collect::<BTreeMap<_, _>>();
    trace!("RigidMeshCollection count in Scene: {}", rigid_mesh_collections.len());

    let mut transaction = Transaction::record(renderer);

    // Create the instances that are requested in the SceneDescription
    for rigid_mesh_instance in &scene.rigid_mesh_instances {
        let rigid_mesh_collection = rigid_mesh_collections
            .get(&rigid_mesh_instance.path)
            .expect("Failed to find rigid mesh collection");
        let position = nalgebra::convert(Translation3::new(
            rigid_mesh_instance.position.x,
            rigid_mesh_instance.position.y,
            rigid_mesh_instance.position.z,
        ));
        let _rigid_mesh_instance_collection = RigidMeshInstanceCollection::from_rigid_mesh_collection(
            &rigid_mesh_collection,
            element_group.rigid_meshes(),
            &mut instance_group,
            &mut transaction,
            &position,
        )
        .expect("Failed to create RigidMeshInstanceCollection");
    }
    trace!("RigidMeshInstanceCollection count in Scene: {}", rigid_mesh_collections.len());

    transaction.finish();

    Ok(())
}
