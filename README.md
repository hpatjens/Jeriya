# Jeriya

![workflow](https://github.com/hpatjens/Jeriya/actions/workflows/build.yml/badge.svg)
![workflow](https://github.com/hpatjens/Jeriya/actions/workflows/fmt.yml/badge.svg)
![workflow](https://github.com/hpatjens/Jeriya/actions/workflows/clippy.yml/badge.svg)
![workflow](https://github.com/hpatjens/Jeriya/actions/workflows/doc.yml/badge.svg)
![workflow](https://github.com/hpatjens/Jeriya/actions/workflows/examples.yml/badge.svg)
[![codecov](https://codecov.io/gh/hpatjens/Jeriya/branch/main/graph/badge.svg?token=JZ0PDV414L)](https://codecov.io/gh/hpatjens/Jeriya)

## Experimental Renderer

Jeriya is an experimental renderer in its infancy.

The following image shows a test scene with a point cloud representation of the Sponza model and primitives rendered in immediate mode.

![Image-0.1.0](docs/image-0.3.0.jpg)

## API

```rust
use std::sync::Arc;

use jeriya_backend_ash::AshBackend;
use jeriya_shared::{
    nalgebra::{self, Vector3, Translation3, Rotation3},
    winit::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,

    },
    FrameRate, RendererConfig, WindowConfig, debug_info,
};
use jeriya_backend::{
    transactions::Transaction,
    elements::{
        camera::{Camera, CameraProjection},
        element_group::ElementGroup,
        helper::{rigid_mesh_collection::RigidMeshCollection, rigid_mesh_instance_collection::RigidMeshInstanceCollection},
        rigid_mesh::RigidMesh,
    },
    instances::{
        camera_instance::{CameraInstance, CameraTransform},
        instance_group::InstanceGroup,
        rigid_mesh_instance::RigidMeshInstance,
    },
    resources::{mesh_attributes::MeshAttributes, resource_group::ResourceGroup},
};
use jeriya_content::{asset_importer::AssetImporter, model::ModelAsset};

// Create Window
let event_loop = EventLoop::new().unwrap();
event_loop.set_control_flow(ControlFlow::Poll);
let window = WindowBuilder::new()
    .with_title("Example")
    .with_inner_size(LogicalSize::new(640.0, 480.0))
    .build(&event_loop)
    .unwrap();

// Create AssetImporter
let asset_importer = Arc::new(AssetImporter::default_from("../assets/processed").unwrap());

// Create Renderer
let renderer = jeriya::Renderer::<AshBackend>::builder()
    .add_renderer_config(RendererConfig::default())
    .add_asset_importer(asset_importer)
    .add_windows(&[
        WindowConfig {
            window: &window,
            frame_rate: FrameRate::Unlimited,
        },
    ])
    .build()
    .unwrap();

// Containers in which GPU resources are managed
let mut resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
let mut element_group = ElementGroup::new(&renderer, debug_info!("my_element_group"));
let mut instance_group = InstanceGroup::new(&renderer, debug_info!("my_instance_group"));

let mut transaction = Transaction::record(&renderer);

// Setup Camera
let camera_builder = Camera::builder()
    .with_projection(CameraProjection::Perspective {
        fov: 90.0,
        aspect: 1.0,
        near: 0.1,
        far: 100.0,
    });
let camera_handle = element_group
    .cameras()
    .mutate_via(&mut transaction)
    .insert_with(camera_builder)
    .unwrap();
let camera = element_group.cameras().get(&camera_handle).unwrap();
let camera_instance_builder = CameraInstance::builder()
    .with_camera(camera)
    .with_transform(CameraTransform {
        position: Vector3::new(0.0, 0.0, -2.0),
        forward: Vector3::new(0.0, 0.0, 1.0),
        up: Vector3::new(0.0, -1.0, 0.0),
    });
let camera_instance_handle = instance_group
    .camera_instances()
    .mutate_via(&mut transaction)
    .insert_with(camera_instance_builder)
    .unwrap();
let camera_instance = instance_group
    .camera_instances()
    .get(&camera_instance_handle)
    .unwrap();
renderer.set_active_camera(window.id(), camera_instance).unwrap();

// Load model
let suzanne = ModelAsset::import("../sample_assets/models/suzanne.glb").unwrap();
let mesh = &suzanne.meshes[1].simple_mesh;

// Copy Vertex Data to GPU
let mesh_attributes_builder = MeshAttributes::builder()
    .with_vertex_positions(mesh.vertex_positions.clone())
    .with_vertex_normals(mesh.vertex_normals.clone())
    .with_indices(mesh.indices.clone());
let mesh_attributes = resource_group
    .mesh_attributes()
    .insert_with(mesh_attributes_builder)
    .unwrap();

// Setup Mesh
let rigid_mesh_builder = RigidMesh::builder().with_mesh_attributes(mesh_attributes);
let rigid_mesh_handle = element_group
    .rigid_meshes()
    .mutate_via(&mut transaction)
    .insert_with(rigid_mesh_builder)
    .unwrap();
let rigid_mesh = element_group.rigid_meshes().get(&rigid_mesh_handle).unwrap();
let rigid_mesh_instance_builder = RigidMeshInstance::builder()
    .with_rigid_mesh(rigid_mesh)
    .with_transform(nalgebra::convert(
        Translation3::new(0.0, -1.0, 0.0) * 
        Rotation3::from_euler_angles(0.0, -std::f32::consts::FRAC_PI_2, 0.0)
    ));
instance_group
    .rigid_mesh_instances()
    .mutate_via(&mut transaction)
    .insert_with(rigid_mesh_instance_builder)
    .unwrap();

// Apply the change to the state.
transaction.finish();

// Returning here so that this code doesn't run indefinitely in the tests.
return;

event_loop.run(move |event, target| {
    match event {
        Event::WindowEvent { event: WindowEvent::CloseRequested, window_id } => {
            if window_id == window.id() { 
                target.exit();
            }
        },
        _ => (),
    }
});
```