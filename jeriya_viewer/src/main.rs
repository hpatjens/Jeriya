mod camera_controller;

use std::{
    f32::consts::TAU,
    io,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use clap::{Parser, ValueEnum};
use color_eyre as ey;
use ey::eyre::{Context, ContextCompat};
use jeriya::Renderer;
use jeriya_backend::{
    elements::{
        camera::{Camera, CameraProjection},
        element_group::ElementGroup,
        helper::{rigid_mesh_collection::RigidMeshCollection, rigid_mesh_instance_collection::RigidMeshInstanceCollection},
        point_cloud,
        rigid_mesh::{MeshRepresentation, RigidMesh},
    },
    immediate::{ImmediateRenderingFrame, LineConfig, LineList, LineStrip, Timeout, TriangleConfig, TriangleList, TriangleStrip},
    instances::{
        camera_instance::CameraInstance, instance_group::InstanceGroup, point_cloud_instance::PointCloudInstance,
        rigid_mesh_instance::RigidMeshInstance,
    },
    resources::{mesh_attributes::MeshAttributes, point_cloud_attributes::PointCloudAttributes, resource_group::ResourceGroup},
    transactions::Transaction,
    Backend,
};
use jeriya_backend_ash::AshBackend;
use jeriya_content::{
    asset_importer::{AssetImporter, FileSystem},
    asset_processor::AssetProcessor,
    common::Directories,
    model::ModelAsset,
    point_cloud::clustered_point_cloud::ClusteredPointCloudAsset,
    shader::{import_shader, ShaderAsset},
};
use jeriya_shared::{
    debug_info,
    log::{self, error, info},
    nalgebra::{self, Matrix4, Scale3, Translation3, Vector2, Vector3, Vector4},
    parking_lot::Mutex,
    spin_sleep,
    winit::{
        dpi::LogicalSize,
        event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        keyboard::{Key, NamedKey},
        window::WindowBuilder,
    },
    FrameRate, RendererConfig, WindowConfig,
};

use crate::camera_controller::CameraController;

/// Shows how the immediate rendering API can be used.
fn immediate_rendering<B>(
    renderer: &Renderer<B>,
    update_loop_frame_index: u64,
    _update_framerate: f64,
    t: Duration,
    _dt: Duration,
) -> jeriya_backend::Result<()>
where
    B: Backend,
{
    let immediate_rendering_frame_config = ImmediateRenderingFrame::new("main_loop", update_loop_frame_index, Timeout::Infinite);

    let immediate_command_buffer_builder = renderer.create_immediate_command_buffer_builder(debug_info!("my_command_buffer"))?;

    // Grid on the floor
    const GRID_STEPS: usize = 10;
    const GRID_EXTENT: f32 = 1.0; // half the length of the line
    const GRID_STEP_SIZE: f32 = 2.0 * GRID_EXTENT / GRID_STEPS as f32;
    let line_list = {
        let n = GRID_STEPS + 1;
        let mut positions = Vec::with_capacity(2 * n);
        for x in 0..n {
            let line_x = x as f32 * GRID_STEP_SIZE - GRID_EXTENT;
            positions.extend(&[Vector3::new(line_x, 0.0, -GRID_EXTENT), Vector3::new(line_x, 0.0, GRID_EXTENT)]);
        }
        for z in 0..n {
            let line_z = z as f32 * GRID_STEP_SIZE - GRID_EXTENT;
            positions.extend(&[Vector3::new(-GRID_EXTENT, 0.0, line_z), Vector3::new(GRID_EXTENT, 0.0, line_z)]);
        }
        LineList::new(
            positions,
            LineConfig {
                color: Vector4::new(0.7, 0.7, 0.9, 1.0),
                ..LineConfig::default()
            },
        )
    };

    // Vertical lines on the corners of the grid
    const CORNER_LINE_LENGTH: f32 = 0.2;
    let corner_lines = {
        let mut positions = Vec::with_capacity(8);
        positions.extend(&[
            Vector3::new(-GRID_EXTENT, 0.0, -GRID_EXTENT),
            Vector3::new(-GRID_EXTENT, CORNER_LINE_LENGTH, -GRID_EXTENT),
            Vector3::new(-GRID_EXTENT, 0.0, GRID_EXTENT),
            Vector3::new(-GRID_EXTENT, CORNER_LINE_LENGTH, GRID_EXTENT),
            Vector3::new(GRID_EXTENT, 0.0, -GRID_EXTENT),
            Vector3::new(GRID_EXTENT, CORNER_LINE_LENGTH, -GRID_EXTENT),
            Vector3::new(GRID_EXTENT, 0.0, GRID_EXTENT),
            Vector3::new(GRID_EXTENT, CORNER_LINE_LENGTH, GRID_EXTENT),
        ]);
        LineList::new(
            positions,
            LineConfig {
                color: Vector4::new(0.7, 0.7, 0.9, 1.0),
                ..LineConfig::default()
            },
        )
    };

    // Cirlce around the grid
    const CIRCLE_STEPS: usize = 128;
    let circle_extent = (2.0 * GRID_EXTENT * GRID_EXTENT).sqrt();
    let line_strip = {
        let n = CIRCLE_STEPS + 1;
        let mut positions = Vec::with_capacity(n);
        for i in 0..n {
            let angle = i as f32 / CIRCLE_STEPS as f32 * TAU;
            let z = angle.cos() * circle_extent;
            let x = angle.sin() * circle_extent;
            positions.push(Vector3::new(x, 0.0, z));
        }
        LineStrip::new(
            positions,
            LineConfig {
                color: Vector4::new(0.8, 0.8, 1.0, 1.0),
                line_width: 5.0,
            },
        )
    };

    // Moving line around circle
    let line_strip_turning = {
        let segment0 = (t.as_secs() % CIRCLE_STEPS as u64) as usize;
        let segment1 = ((t.as_secs() + 1) % CIRCLE_STEPS as u64) as usize;
        let angle0 = segment0 as f32 / CIRCLE_STEPS as f32 * TAU;
        let angle1 = segment1 as f32 / CIRCLE_STEPS as f32 * TAU;
        let z0 = angle0.cos() * circle_extent;
        let x0 = angle0.sin() * circle_extent;
        let z1 = angle1.cos() * circle_extent;
        let x1 = angle1.sin() * circle_extent;
        let position0 = Vector3::new(x0, 0.0, z0);
        let position1 = Vector3::new(x1, 0.0, z1);
        LineStrip::new(
            vec![position0, position1],
            LineConfig {
                color: Vector4::new(1.0, 0.0, 0.0, 1.0),
                line_width: 4.0,
            },
        )
    };
    let triangle_list = TriangleList::new(
        vec![
            Vector3::new(-0.8, -0.8, 0.0),
            Vector3::new(-0.8, -0.6, 0.0),
            Vector3::new(-0.6, -0.7, 0.0),
            Vector3::new(-0.5, -0.7, 0.0),
            Vector3::new(-0.5, -0.5, 0.0),
            Vector3::new(-0.2, -0.6, 0.0),
        ],
        TriangleConfig {
            color: Vector4::new(1.0, 0.3, 0.7, 1.0),
        },
    );
    let triangle_strip = TriangleStrip::new(
        vec![
            Vector3::new(0.7, -0.8, 0.0),
            Vector3::new(0.3, -0.8, 0.0),
            Vector3::new(0.7, -0.6, 0.0),
            Vector3::new(0.3, -0.5, 0.0),
        ],
        TriangleConfig {
            color: Vector4::new(1.0, 1.0, 0.2, 1.0),
        },
    );

    let immediate_command_buffer = immediate_command_buffer_builder
        .push_line_lists(&[line_list, corner_lines])?
        .push_line_strips(&[line_strip, line_strip_turning])?
        .matrix(Matrix4::new_scaling(0.5))?
        .push_triangle_lists(&[triangle_list])?
        .push_triangle_strips(&[triangle_strip])?
        .build()?;

    renderer.render_immediate_command_buffer(&immediate_rendering_frame_config, immediate_command_buffer)?;

    Ok(())
}

fn setup_asset_processor() -> ey::Result<AssetProcessor> {
    let directories = Directories::create_all_dir("assets/unprocessed", "assets/processed")
        .wrap_err("Failed to create Directories for AssetProcessor")?;
    let asset_processor = AssetProcessor::new(&directories, 4)
        .wrap_err("Failed to create AssetProcessor")?
        .register("glb", Box::new(jeriya_content::model::process_model))
        .register("vert", Box::new(jeriya_content::shader::process_shader))
        .register("frag", Box::new(jeriya_content::shader::process_shader))
        .register("comp", Box::new(jeriya_content::shader::process_shader));
    asset_processor.set_active(true)?;
    Ok(asset_processor)
}

#[derive(ValueEnum, Debug, Clone)]
enum FileType {
    Model,
    PointCloud,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CommandLineArguments {
    /// Path to the file to open
    #[arg(default_value_t = String::from("sample_assets/models/suzanne.glb"))]
    // not a PathBuf because PathBuf does not implement Display
    path: String,

    /// Type of the file to open
    #[arg(long, short, default_value = "model")]
    file_type: FileType,

    /// Enable meshlet rendering
    #[arg(long, short, default_value_t = true)]
    enable_meshlet_rendering: bool,

    /// Scale of the model
    #[arg(long, short, default_value_t = 1.0)]
    scale: f32,
}

fn main() -> ey::Result<()> {
    // Parse command line arguments
    let command_line_arguments = CommandLineArguments::parse();

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

    // Create Windows
    let event_loop = EventLoop::new().wrap_err("Failed to create EventLoop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let window1 = WindowBuilder::new()
        .with_title("Window 1")
        .with_inner_size(LogicalSize::new(1000.0, 1000.0))
        .build(&event_loop)
        .wrap_err("Failed to create window 1")?;
    let window2 = WindowBuilder::new()
        .with_title("Window 2")
        .with_inner_size(LogicalSize::new(1000.0, 1000.0))
        .build(&event_loop)
        .wrap_err("Failed to create window 2")?;

    // Setup Content Pipeline
    let _asset_processor = setup_asset_processor()?;
    let asset_importer = Arc::new(AssetImporter::default_from("assets/processed").wrap_err("Failed to create AssetImporter")?);

    // Create Renderer
    let renderer = jeriya::Renderer::<AshBackend>::builder()
        .add_renderer_config(RendererConfig::normal())
        .add_asset_importer(asset_importer)
        .add_windows(&[
            WindowConfig {
                window: &window1,
                frame_rate: FrameRate::Unlimited,
            },
            WindowConfig {
                window: &window2,
                frame_rate: FrameRate::Limited(60),
            },
        ])
        .build()
        .wrap_err("Failed to create renderer")?;

    // Containers in which manage the GPU resources
    let mut resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
    let mut element_group = ElementGroup::new(&renderer, debug_info!("my_element_group"));
    let mut instance_group = InstanceGroup::new(&renderer, debug_info!("my_instance_group"));

    // Load models
    let cube_model = ModelAsset::import("sample_assets/models/rotated_cube.glb").wrap_err("Failed to import model")?;

    // Create MeshAttributes for the model
    //
    // This will upload the vertex positions and normals to the GPU asynchronously. When the upload
    // is done a MeshAttributes value will be uploaded to the GPU so that RigidMeshes can reference
    // the vertex data.
    let mesh = &cube_model.meshes.get(0).unwrap().simple_mesh;
    let mesh_attributes_builder = MeshAttributes::builder()
        .with_vertex_positions(mesh.vertex_positions.clone())
        .with_vertex_normals(mesh.vertex_normals.clone())
        .with_indices(mesh.indices.clone())
        .with_debug_info(debug_info!("my_mesh"));
    let mesh_attributes = resource_group.mesh_attributes().insert_with(mesh_attributes_builder).unwrap();

    // Create a Transaction to record changes to the ElementGroup and InstanceGroup.
    //
    // Transactions are a sequence of state changes that will be applied on the GPU as one operation.
    // This is useful for batching changes together to reduce the number of GPU operations and making
    // sure that the GPU is not in an inconsistent state. All changes to the ElementGroup and
    // InstanceGroup must be done via a Transaction.
    let mut transaction = Transaction::record(&renderer);

    // Create Camera for Window 1
    //
    // The camera itself cannot be used to create a view onto the scene. Instead it only defines the
    // properties of the camera. To create a view onto the scene a CameraInstance must be created that
    // references a Camera.
    let camera1_builder = Camera::builder()
        .with_projection(CameraProjection::Orthographic {
            left: -5.0,
            right: 5.0,
            bottom: 5.0,
            top: -5.0,
            near: -5.0,
            far: 5.0,
        })
        .with_debug_info(debug_info!("my_camera1"));
    let camera1_handle = element_group.cameras().mutate_via(&mut transaction).insert_with(camera1_builder)?;

    // Create Camera for Window 2
    let camera2_builder = Camera::builder()
        .with_projection(CameraProjection::Perspective {
            fov: 90.0,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        })
        .with_debug_info(debug_info!("my_camera2"));
    let camera2_handle = element_group.cameras().mutate_via(&mut transaction).insert_with(camera2_builder)?;

    // Create CameraInstance for Window1
    let camera1_instance_builder = CameraInstance::builder()
        .with_camera(element_group.cameras().get(&camera1_handle).wrap_err("Failed to find camera")?)
        .with_debug_info(debug_info!("my_camera_instance"));
    let camera1_instance_handle = instance_group
        .camera_instances()
        .mutate_via(&mut transaction)
        .insert_with(camera1_instance_builder)?;
    let camera1_instance = instance_group
        .camera_instances()
        .get(&camera1_instance_handle)
        .wrap_err("Failed to find camera instance")?;
    renderer
        .set_active_camera(window1.id(), camera1_instance)
        .wrap_err("Failed to set active camera")?;

    // Create CameraInstance for Window2
    let camera2_instance_builder = CameraInstance::builder()
        .with_camera(element_group.cameras().get(&camera2_handle).wrap_err("Failed to find camera")?)
        .with_debug_info(debug_info!("my_camera_instance"));
    let camera2_instance_handle = instance_group
        .camera_instances()
        .mutate_via(&mut transaction)
        .insert_with(camera2_instance_builder)?;
    let camera2_instance = instance_group
        .camera_instances()
        .get(&camera2_instance_handle)
        .wrap_err("Failed to find camera instance")?;
    renderer
        .set_active_camera(window2.id(), camera2_instance)
        .wrap_err("Failed to set active camera")?;

    // Create RigidMesh
    //
    // A RigidMesh is a mesh that is not animated. It can be instanced multiple times in the scene. To
    // define the appearance of the RigidMesh, a MeshAttributes value must be referenced. The RigidMesh
    // itself is not displayed in the scene. Instead RigidMeshInstances must be created that reference
    // a RigidMesh.
    let rigid_mesh_builder = RigidMesh::builder()
        .with_mesh_attributes(mesh_attributes)
        .with_preferred_mesh_representation(MeshRepresentation::Simple)
        .with_debug_info(debug_info!("my_rigid_mesh"));
    let rigid_mesh_handle = element_group
        .rigid_meshes()
        .mutate_via(&mut transaction)
        .insert_with(rigid_mesh_builder)?;

    // Finishing the Transaction will queue all changes to be applied in the next frame.
    transaction.finish();

    let resource_group = Arc::new(Mutex::new(resource_group));
    let element_group = Arc::new(Mutex::new(element_group));
    let instance_group = Arc::new(Mutex::new(instance_group));

    let renderer2 = Arc::clone(&renderer);
    let resource_group2 = Arc::clone(&resource_group);
    let element_group2 = Arc::clone(&element_group);
    let instance_group2 = Arc::clone(&instance_group);
    thread::spawn(move || {
        match command_line_arguments.file_type {
            FileType::Model => {
                let main_model = ModelAsset::import(command_line_arguments.path)
                    .wrap_err("Failed to import model")
                    .expect("Failed to import model");

                let mut resource_group = resource_group2.lock();
                let mut element_group = element_group2.lock();
                let mut instance_group = instance_group2.lock();

                let mut transaction = Transaction::record(&renderer2);

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
                let clustered_point_cloud = ClusteredPointCloudAsset::deserialize_from_file(&command_line_arguments.path)
                    .wrap_err("Failed to deserialize PointCloud")
                    .expect("Failed to deserialize PointCloud");
                info!("PointCloud to view: {clustered_point_cloud:?}");

                let mut resource_group = resource_group2.lock();
                let mut element_group = element_group2.lock();
                let mut instance_group = instance_group2.lock();

                // Create PointCloudAttributes
                let point_cloud_attributes_builder = PointCloudAttributes::builder()
                    .with_debug_info(debug_info!("my_point_cloud_attributes"))
                    .with_pages(clustered_point_cloud.pages().to_vec())
                    .with_root_cluster_index(clustered_point_cloud.root_cluster_index().clone());
                let point_cloud_attributes = resource_group
                    .point_cloud_attributes()
                    .insert_with(point_cloud_attributes_builder)
                    .expect("Failed to insert PointCloudAttributes");

                let mut transaction = Transaction::record(&renderer2);

                // Create PointCloud
                let point_cloud_builder = point_cloud::PointCloud::builder()
                    .with_point_cloud_attributes(point_cloud_attributes)
                    .with_debug_info(debug_info!("my_point_cloud"));
                let point_cloud = element_group
                    .point_clouds()
                    .mutate_via(&mut transaction)
                    .insert_with(point_cloud_builder)
                    .expect("Failed to insert PointCloud");

                // Create PointCloudInstance
                let s = command_line_arguments.scale;
                let point_cloud_instance_builder = PointCloudInstance::builder()
                    .with_point_cloud(element_group.point_clouds().get(&point_cloud).unwrap())
                    .with_transform(Scale3::new(s, s, s).into())
                    .with_debug_info(debug_info!("my_point_cloud_instance"));
                let _point_cloud_instance = instance_group
                    .point_cloud_instances()
                    .mutate_via(&mut transaction)
                    .insert_with(point_cloud_instance_builder)
                    .expect("Failed to insert PointCloudInstance");

                transaction.finish();
            }
        }
    });

    let mut camera_controller2 = CameraController::new(camera_controller::Config {
        rotate_theta_speed_keyboard: 2.0,
        rotate_theta_speed_mouse_cursor: 0.2,
        rotate_phi_speed_keyboard: 2.0,
        rotate_phi_speed_mouse_cursor: 0.2,
        zoom_speed_mouse_wheel: 0.4,
        zoom_speed_keyboard: 5.0,
    });

    const UPDATE_FRAMERATE: u32 = 60;
    let loop_start_time = Instant::now();
    let mut last_frame_start_time = Instant::now();
    let mut update_loop_frame_index = 0;
    let mut mesh_count = 0;
    let mut last_mesh_insert_t = Duration::from_secs(0);
    let mut loop_helper = spin_sleep::LoopHelper::builder().build_with_target_rate(UPDATE_FRAMERATE as f64);
    event_loop
        .run(move |event, event_loop_window_target| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => event_loop_window_target.exit(),
            Event::WindowEvent { window_id, event } => {
                if window_id == window2.id() {
                    match event {
                        WindowEvent::CloseRequested => event_loop_window_target.exit(),
                        WindowEvent::KeyboardInput { event, .. } => match event.logical_key {
                            Key::Named(NamedKey::ArrowRight) => camera_controller2.set_rotating_right(event.state == ElementState::Pressed),
                            Key::Named(NamedKey::ArrowLeft) => camera_controller2.set_rotating_left(event.state == ElementState::Pressed),
                            Key::Named(NamedKey::ArrowUp) => camera_controller2.set_rotating_up(event.state == ElementState::Pressed),
                            Key::Named(NamedKey::ArrowDown) => camera_controller2.set_rotating_down(event.state == ElementState::Pressed),
                            Key::Named(NamedKey::PageUp) => camera_controller2.set_zooming_in(event.state == ElementState::Pressed),
                            Key::Named(NamedKey::PageDown) => camera_controller2.set_zooming_out(event.state == ElementState::Pressed),
                            _ => {}
                        },
                        WindowEvent::CursorMoved { position, .. } => {
                            camera_controller2.set_cursor_position(Vector2::new(position.x as f32, position.y as f32));
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            if window_id == window2.id() {
                                match delta {
                                    MouseScrollDelta::LineDelta(_x, y) => camera_controller2.zoom_out(-y as f32),
                                    MouseScrollDelta::PixelDelta(delta) => camera_controller2.zoom_out(-delta.y as f32),
                                }
                            }
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            camera_controller2.set_cursor_rotation_active(button == MouseButton::Left && state == ElementState::Pressed);
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                loop_helper.loop_start();
                let frame_start_time = Instant::now();
                let t = frame_start_time - loop_start_time;
                let dt = frame_start_time - last_frame_start_time;

                let mut transaction = Transaction::record(&renderer);

                camera_controller2
                    .update(dt, &mut transaction, &mut instance_group.lock(), camera2_instance_handle)
                    .expect("Failed to update camera controller");

                if mesh_count < 10 && (t - last_mesh_insert_t).as_secs() > 1 {
                    last_mesh_insert_t = t;
                    let radius = 3.5;
                    let position = Vector3::new(t.as_secs_f32().sin() * radius, 0.0, t.as_secs_f32().cos() * radius);
                    let mut element_group = element_group.lock();
                    let rigid_mesh = element_group
                        .rigid_meshes()
                        .get(&rigid_mesh_handle)
                        .expect("Failed to find rigid mesh");
                    let rigid_mesh_instance_builder = RigidMeshInstance::builder()
                        .with_rigid_mesh(rigid_mesh)
                        .with_transform(nalgebra::convert(Translation3::from(position)))
                        .with_debug_info(debug_info!("my_rigid_mesh_instance"));
                    instance_group
                        .lock()
                        .rigid_mesh_instances()
                        .mutate_via(&mut transaction)
                        .insert_with(rigid_mesh_instance_builder)
                        .expect("Failed to insert rigid mesh instance");
                    mesh_count += 1;
                }

                transaction.finish();

                if let Err(err) = immediate_rendering(&renderer, update_loop_frame_index, UPDATE_FRAMERATE as f64, t, dt) {
                    error!("Failed to do immediate rendering: {}", err);
                    event_loop_window_target.exit();
                    return;
                }

                update_loop_frame_index += 1;
                last_frame_start_time = frame_start_time;
                loop_helper.loop_sleep();
            }
            _ => (),
        })
        .wrap_err("Running the EventLoop failed")?;

    Ok(())
}
