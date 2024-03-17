use std::{collections::BTreeMap, path::Path, sync::Arc};

use color_eyre::eyre::Context;
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
use jeriya_content::{
    model::ModelAsset,
    point_cloud::{self, clustered_point_cloud::ClusteredPointCloudAsset},
};
use jeriya_shared::{
    debug_info,
    log::{info, trace},
    nalgebra::{self, Scale3, Translation3},
    parking_lot::Mutex,
};

use crate::{scene_description::SceneDescription, FileType};

pub fn load_scene<B: Backend>(
    path: impl AsRef<Path>,
    file_type: FileType,
    scale: f32,
    renderer: &Arc<Renderer<B>>,
    resource_group: &Arc<Mutex<ResourceGroup>>,
    element_group: &Arc<Mutex<ElementGroup>>,
    instance_group: &Arc<Mutex<InstanceGroup>>,
) {
    match file_type {
        FileType::Scene => {
            let scene_base_path = Path::new(path.as_ref()).parent().expect("failed to get parent path of scene");
            let scene = SceneDescription::import(path.as_ref()).unwrap();
            info!("Scene to view: {scene:?}");

            let mut resource_group = resource_group.lock();
            let mut element_group = element_group.lock();
            let mut instance_group = instance_group.lock();

            let rigid_meshes = scene
                .rigid_mesh_instances
                .iter()
                .map(|instance| {
                    let absolute_path = if instance.path.is_absolute() {
                        instance.path.clone()
                    } else {
                        scene_base_path.join(&instance.path)
                    };
                    (instance.path.clone(), absolute_path)
                })
                .collect::<BTreeMap<_, _>>();
            trace!("RigidMesh count in Scene: {}", rigid_meshes.len());
            let rigid_mesh_collections = rigid_meshes
                .iter()
                .map(|(path, absolute_path)| {
                    let model = ModelAsset::import(absolute_path).unwrap();
                    let rigid_mesh_collection = RigidMeshCollection::from_model(
                        &model,
                        &mut resource_group,
                        &mut element_group,
                        &mut Transaction::record(renderer),
                    )
                    .unwrap();
                    (path, rigid_mesh_collection)
                })
                .collect::<BTreeMap<_, _>>();
            trace!("RigidMeshCollection count in Scene: {}", rigid_mesh_collections.len());

            let mut transaction = Transaction::record(renderer);

            for rigid_mesh_instance in &scene.rigid_mesh_instances {
                let rigid_mesh_collection = rigid_mesh_collections
                    .get(&rigid_mesh_instance.path)
                    .expect("Failed to find rigid mesh collection");
                let _rigid_mesh_instance_collection = RigidMeshInstanceCollection::from_rigid_mesh_collection(
                    &rigid_mesh_collection,
                    element_group.rigid_meshes(),
                    &mut instance_group,
                    &mut transaction,
                    &nalgebra::convert(Translation3::new(
                        rigid_mesh_instance.position.x,
                        rigid_mesh_instance.position.y,
                        rigid_mesh_instance.position.z,
                    )),
                )
                .expect("Failed to create RigidMeshInstanceCollection");
            }
            trace!("RigidMeshInstanceCollection count in Scene: {}", rigid_mesh_collections.len());

            transaction.finish();
        }
        FileType::Model => {
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
            let rigid_mesh_collection =
                RigidMeshCollection::from_model(&main_model, &mut resource_group, &mut element_group, &mut transaction)
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
        }
        FileType::PointCloud => {
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
        }
    }
}
