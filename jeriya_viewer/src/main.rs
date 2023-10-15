use std::{
    io,
    time::{Duration, Instant},
};

use clap::Parser;
use color_eyre as ey;
use ey::eyre::{Context, ContextCompat};
use jeriya::Renderer;
use jeriya_backend::{
    elements::{
        camera::{Camera, CameraProjection},
        element_group::ElementGroup,
        helper::{rigid_mesh_collection::RigidMeshCollection, rigid_mesh_instance_collection::RigidMeshInstanceCollection},
        rigid_mesh::RigidMesh,
    },
    immediate::{ImmediateRenderingFrame, LineConfig, LineList, LineStrip, Timeout, TriangleConfig, TriangleList, TriangleStrip},
    instances::{
        camera_instance::{CameraInstance, CameraTransform},
        instance_group::InstanceGroup,
        rigid_mesh_instance::RigidMeshInstance,
    },
    resources::{mesh_attributes::MeshAttributes, resource_group::ResourceGroup},
    transactions::Transaction,
    Backend,
};
use jeriya_backend_ash::AshBackend;
use jeriya_content::{model::Model, AssetImporter, AssetProcessor, Directories, FileSystem};
use jeriya_shared::{
    debug_info,
    log::{self, error},
    nalgebra::{self, Matrix4, Translation3, Vector3, Vector4},
    spin_sleep,
    winit::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        window::WindowBuilder,
    },
    FrameRate, RendererConfig, WindowConfig,
};

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

    let line_list = LineList::new(
        vec![Vector3::new(-0.5, 0.2, 0.0), Vector3::new(0.8, 0.8, 0.0)],
        LineConfig {
            color: Vector4::new(0.1, 0.1, 0.7, 1.0),
            ..LineConfig::default()
        },
    );
    let line_strip = LineStrip::new(
        vec![
            Vector3::new(-0.5, 0.8, 0.0),
            Vector3::new(-0.2, 0.8, 0.0),
            Vector3::new(-0.3, 0.5, 0.0),
            Vector3::new(-0.7, 0.4, 0.0),
        ],
        LineConfig {
            color: Vector4::new(0.8, 1.0, 0.4, 1.0),
            line_width: 5.0,
        },
    );
    let line_strip_turning = {
        let x = t.as_secs_f32().sin() * 0.5;
        let y = t.as_secs_f32().cos() * 0.5;
        let offset = 1.5;
        LineStrip::new(
            vec![Vector3::new(x, offset + y, 0.0), Vector3::new(-x, offset - y, 0.0)],
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
        .push_line_lists(&[line_list])?
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
        .register("glb", Box::new(jeriya_content::model::process_model));
    asset_processor.set_active(true)?;
    Ok(asset_processor)
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CommandLineArguments {
    /// Path to the file to open
    #[arg(default_value_t = String::from("sample_assets/suzanne.glb"))] // not a PathBuf because PathBuf does not implement Display
    path: String,

    /// Enable meshlet rendering
    #[arg(long, short, default_value_t = true)]
    enable_meshlet_rendering: bool,
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
    let event_loop = EventLoop::new();
    let window1 = WindowBuilder::new()
        .with_title("Example")
        .with_inner_size(LogicalSize::new(640.0, 480.0))
        .build(&event_loop)
        .wrap_err("Failed to create window 1")?;
    let window2 = WindowBuilder::new()
        .with_title("Example")
        .with_inner_size(LogicalSize::new(640.0, 480.0))
        .build(&event_loop)
        .wrap_err("Failed to create window 2")?;

    // Create Renderer
    let renderer = jeriya::Renderer::<AshBackend>::builder()
        .add_renderer_config(RendererConfig::default())
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

    // Setup Content Pipeline
    let _asset_processor = setup_asset_processor()?;
    let import_source = FileSystem::new("assets/unprocessed").wrap_err("Failed to create ImportSource for AssetImporter")?;
    let _asset_importer = AssetImporter::new(import_source, 4).wrap_err("Failed to create AssetImporter")?;

    // Containers in which manage the GPU resources
    let mut resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
    let mut element_group = ElementGroup::new(&renderer, debug_info!("my_element_group"));
    let mut instance_group = InstanceGroup::new(&renderer, debug_info!("my_instance_group"));

    // Load models
    let cube_model = Model::import("sample_assets/rotated_cube.glb").wrap_err("Failed to import model")?;
    let main_model = Model::import(command_line_arguments.path).wrap_err("Failed to import model")?;

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
        .with_debug_info(debug_info!("my_rigid_mesh"));
    let rigid_mesh_handle = element_group
        .rigid_meshes()
        .mutate_via(&mut transaction)
        .insert_with(rigid_mesh_builder)?;
    let rigid_mesh = element_group.rigid_meshes().get(&rigid_mesh_handle).unwrap();

    // Create RigidMeshInstance
    let rigid_mesh_instance_builder = RigidMeshInstance::builder()
        .with_rigid_mesh(rigid_mesh)
        .with_transform(nalgebra::convert(Translation3::new(1.5, 0.0, 0.0)))
        .with_debug_info(debug_info!("my_rigid_mesh_instance"));
    instance_group
        .rigid_mesh_instances()
        .mutate_via(&mut transaction)
        .insert_with(rigid_mesh_instance_builder)?;

    // Create a RigidMesh from model
    //
    // A RigidMeshCollection can be used to create multiple RigidMeshes from a single Model. To display
    // the RigidMeshes in the scene, RigidMeshInstances must be created that reference the RigidMeshes.
    // A RigidMeshInstanceCollection can be used for that.
    let rigid_mesh_collection = RigidMeshCollection::from_model(&main_model, &mut resource_group, &mut element_group, &mut transaction)?;
    let _rigid_mesh_instance_collection = RigidMeshInstanceCollection::from_rigid_mesh_collection(
        &rigid_mesh_collection,
        element_group.rigid_meshes(),
        &mut instance_group,
        &mut transaction,
        &nalgebra::convert(Translation3::new(-1.5, 0.0, 0.0)),
    )?;

    // Finishing the Transaction will queue all changes to be applied in the next frame.
    transaction.finish();

    const UPDATE_FRAMERATE: u32 = 60;
    let loop_start_time = Instant::now();
    let mut last_frame_start_time = Instant::now();
    let mut update_loop_frame_index = 0;
    let mut mesh_count = 0;
    let mut last_mesh_insert_t = Duration::from_secs(0);
    let mut loop_helper = spin_sleep::LoopHelper::builder().build_with_target_rate(UPDATE_FRAMERATE as f64);
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window1.id() => control_flow.set_exit(),
            Event::MainEventsCleared => {
                loop_helper.loop_start();
                let frame_start_time = Instant::now();
                let t = frame_start_time - loop_start_time;
                let dt = frame_start_time - last_frame_start_time;

                let mut transaction = Transaction::record(&renderer);

                {
                    let position = Vector3::new(t.as_secs_f32().sin() * 0.3, t.as_secs_f32().cos() * 0.3, 0.5);
                    instance_group
                        .camera_instances()
                        .get_mut(&camera1_instance_handle)
                        .expect("Failed to find camera instance")
                        .mutate_via(&mut transaction)
                        .set_transform(CameraTransform {
                            position,
                            ..Default::default()
                        });
                }

                {
                    let distance = 4.0;
                    let position = Vector3::new(t.as_secs_f32().sin() * distance, 3.0, t.as_secs_f32().cos() * distance);
                    instance_group
                        .camera_instances()
                        .get_mut(&camera2_instance_handle)
                        .expect("Failed to find camera instance")
                        .mutate_via(&mut transaction)
                        .set_transform(CameraTransform::new(position, -position.normalize(), Vector3::new(0.0, -1.0, 0.0)));
                }

                if mesh_count < 10 && (t - last_mesh_insert_t).as_secs() > 1 {
                    last_mesh_insert_t = t;
                    let radius = 3.5;
                    let position = Vector3::new(t.as_secs_f32().sin() * radius, 0.0, t.as_secs_f32().cos() * radius);
                    let rigid_mesh = element_group
                        .rigid_meshes()
                        .get(&rigid_mesh_handle)
                        .expect("Failed to find rigid mesh");
                    let rigid_mesh_instance_builder = RigidMeshInstance::builder()
                        .with_rigid_mesh(rigid_mesh)
                        .with_transform(nalgebra::convert(Translation3::from(position)))
                        .with_debug_info(debug_info!("my_rigid_mesh_instance"));
                    instance_group
                        .rigid_mesh_instances()
                        .mutate_via(&mut transaction)
                        .insert_with(rigid_mesh_instance_builder)
                        .expect("Failed to insert rigid mesh instance");
                    mesh_count += 1;
                }

                transaction.finish();

                if let Err(err) = immediate_rendering(&renderer, update_loop_frame_index, UPDATE_FRAMERATE as f64, t, dt) {
                    error!("Failed to do immediate rendering: {}", err);
                    control_flow.set_exit();
                    return;
                }

                update_loop_frame_index += 1;
                last_frame_start_time = frame_start_time;
                loop_helper.loop_sleep();
            }
            _ => (),
        }
    });
}
