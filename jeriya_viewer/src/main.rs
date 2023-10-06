use std::{
    io,
    time::{Duration, Instant},
};

use color_eyre as ey;
use ey::eyre::{eyre, Context};
use gltf::mesh::util::ReadIndices;
use jeriya::Renderer;
use jeriya_backend::{
    elements::{
        element_group::ElementGroup,
        helper::{rigid_mesh_collection::RigidMeshCollection, rigid_mesh_instance_collection::RigidMeshInstanceCollection},
        rigid_mesh::RigidMesh,
    },
    immediate::{ImmediateRenderingFrame, LineConfig, LineList, LineStrip, Timeout, TriangleConfig, TriangleList, TriangleStrip},
    instance_group::InstanceGroup,
    mesh_attributes::MeshAttributes,
    resource_group::ResourceGroup,
    rigid_mesh_instance::RigidMeshInstance,
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

fn load_model() -> ey::Result<Vec<Vector3<f32>>> {
    let (document, buffers, _images) = gltf::import("sample_assets/rotated_cube.glb").wrap_err("Failed to import glTF model")?;
    let mut vertex_positions = Vec::new();
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let temp_vertex_positions = reader.read_positions().expect("no positions in mesh").collect::<Vec<_>>();
            if let Some(indices) = reader.read_indices() {
                match &indices {
                    ReadIndices::U8(iter) => {
                        for index in iter.clone() {
                            vertex_positions.push(temp_vertex_positions[index as usize]);
                        }
                    }
                    ReadIndices::U16(iter) => {
                        for index in iter.clone() {
                            vertex_positions.push(temp_vertex_positions[index as usize]);
                        }
                    }
                    ReadIndices::U32(iter) => {
                        for index in iter.clone() {
                            vertex_positions.push(temp_vertex_positions[index as usize]);
                        }
                    }
                }
            }
        }
    }
    Ok(vertex_positions.into_iter().map(|v| Vector3::new(v[0], v[1], v[2])).collect())
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

fn main() -> ey::Result<()> {
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
    let window_config1 = WindowConfig {
        window: &window1,
        frame_rate: FrameRate::Unlimited,
    };
    let window_config2 = WindowConfig {
        window: &window2,
        frame_rate: FrameRate::Limited(60),
    };
    let renderer = jeriya::Renderer::<AshBackend>::builder()
        .add_renderer_config(RendererConfig {
            maximum_number_of_cameras: 2,
            ..Default::default()
        })
        .add_windows(&[window_config1, window_config2])
        .build()
        .wrap_err("Failed to create renderer")?;

    let _asset_processor = setup_asset_processor()?;

    let import_source = FileSystem::new("assets/unprocessed").wrap_err("Failed to create ImportSource for AssetImporter")?;
    let _asset_importer = AssetImporter::new(import_source, 4).wrap_err("Failed to create AssetImporter")?;

    let handle2 = renderer.active_camera(window2.id()).wrap_err("Failed to get active camera")?;
    let mut cameras = renderer.cameras();
    let mut camera2 = cameras.get_mut(&handle2).ok_or(eyre!("Failed to get camera"))?;
    camera2.set_projection(jeriya_backend::CameraProjection::Perspective {
        fov: 90.0,
        aspect: 1.0,
        near: 0.1,
        far: 100.0,
    });
    drop(cameras);

    let mut resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
    let mut element_group = ElementGroup::new(&renderer, debug_info!("my_element_group"));
    let mut instance_group = InstanceGroup::new(&renderer, debug_info!("my_instance_group"));

    let model = load_model().wrap_err("Failed to load model")?;
    let fake_normals = model.iter().map(|_| Vector3::new(0.0, 1.0, 0.0)).collect::<Vec<_>>();

    let suzanne = Model::import("sample_assets/suzanne.glb").wrap_err("Failed to import model")?;

    let mesh_attributes_builder = MeshAttributes::builder()
        .with_vertex_positions(model.clone())
        .with_vertex_normals(fake_normals.clone())
        .with_debug_info(debug_info!("my_mesh"));
    let mesh_attributes = resource_group.mesh_attributes().insert_with(mesh_attributes_builder).unwrap();

    let mut transaction = Transaction::record(&renderer);
    let rigid_mesh_collection = RigidMeshCollection::from_model(&suzanne, &mut resource_group, &mut element_group, &mut transaction)?;
    let _rigid_mesh_instance_collection = RigidMeshInstanceCollection::from_rigid_mesh_collection(
        &rigid_mesh_collection,
        element_group.rigid_meshes(),
        &mut instance_group,
        &mut transaction,
        &nalgebra::convert(Translation3::new(-1.5, 0.0, 0.0)),
    )?;

    let rigid_mesh_builder = RigidMesh::builder()
        .with_mesh_attributes(mesh_attributes)
        .with_debug_info(debug_info!("my_rigid_mesh"));
    let rigid_mesh_handle = element_group
        .rigid_meshes()
        .mutate_via(&mut transaction)
        .insert_with(rigid_mesh_builder)?;
    let rigid_mesh = element_group.rigid_meshes().get(&rigid_mesh_handle).unwrap();

    let rigid_mesh_instance_builder = RigidMeshInstance::builder()
        .with_rigid_mesh(rigid_mesh)
        .with_transform(nalgebra::convert(Translation3::new(1.5, 0.0, 0.0)))
        .with_debug_info(debug_info!("my_rigid_mesh_instance"));
    instance_group
        .rigid_mesh_instances()
        .mutate_via(&mut transaction)
        .insert_with(rigid_mesh_instance_builder)?;
    transaction.finish();

    const UPDATE_FRAMERATE: u32 = 60;
    let loop_start_time = Instant::now();
    let mut last_frame_start_time = Instant::now();
    let mut update_loop_frame_index = 0;
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

                {
                    let handle = renderer
                        .active_camera(window1.id())
                        .wrap_err("Failed to get active camera")
                        .unwrap();
                    let mut cameras = renderer.cameras();
                    let mut camera = cameras.get_mut(&handle).ok_or(eyre!("Failed to get camera")).unwrap();
                    camera.set_position(Vector3::new(t.as_secs_f32().sin() * 0.3, t.as_secs_f32().cos() * 0.3, 0.0));
                }

                {
                    let handle = renderer
                        .active_camera(window2.id())
                        .wrap_err("Failed to get active camera")
                        .unwrap();
                    let mut cameras = renderer.cameras();
                    let mut camera = cameras.get_mut(&handle).ok_or(eyre!("Failed to get camera")).unwrap();
                    let distance = 3.0;
                    let position = Vector3::new(t.as_secs_f32().sin() * distance, 1.0, t.as_secs_f32().cos() * distance);
                    camera.set_position(position);
                    camera.set_forward(-position.normalize());
                }

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
