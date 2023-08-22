use std::io;

use color_eyre as ey;
use ey::eyre::{eyre, Context};
use gltf::mesh::util::ReadIndices;
use jeriya::Renderer;
use jeriya_backend_ash::AshBackend;
use jeriya_content::{AssetImporter, AssetProcessor, Directories, FileSystem, ImportConfiguration, ProcessConfiguration};
use jeriya_shared::{
    debug_info,
    immediate::{LineConfig, LineList, LineStrip, TriangleConfig, TriangleList, TriangleStrip},
    inanimate_mesh::MeshType,
    log::{self, error},
    nalgebra::{Affine3, Matrix4, Vector3, Vector4},
    winit::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        window::WindowBuilder,
    },
    Backend, InanimateMeshInstance, RendererConfig,
};

/// Shows how the immediate rendering API can be used.
fn immediate_rendering<B>(renderer: &Renderer<B>) -> jeriya_shared::Result<()>
where
    B: Backend,
{
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
        .push_line_strips(&[line_strip])?
        .matrix(Matrix4::new_scaling(0.5))?
        .push_triangle_lists(&[triangle_list])?
        .push_triangle_strips(&[triangle_strip])?
        .build()?;

    renderer.render_immediate_command_buffer(immediate_command_buffer)?;

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
        .level(log::LevelFilter::Debug)
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
    let renderer = jeriya::Renderer::<AshBackend>::builder()
        .add_renderer_config(RendererConfig {
            maximum_number_of_cameras: 2,
            maximum_number_of_inanimate_mesh_instances: 10,
            maximum_number_of_inanimate_meshes: 10,
            ..Default::default()
        })
        .add_windows(&[&window1, &window2])
        .build()
        .wrap_err("Failed to create renderer")?;

    let directories = Directories::create_all_dir("assets/unprocessed", "assets/processed")
        .wrap_err("Failed to create Directories for AssetProcessor")?;
    let mut asset_processor = AssetProcessor::new(&directories, 4).wrap_err("Failed to create AssetProcessor")?;
    asset_processor
        .register(ProcessConfiguration {
            extension: "glb".to_string(),
            processor: Box::new(jeriya_content::model::process_model),
        })
        .unwrap();

    let import_source = FileSystem::new("assets/unprocessed").wrap_err("Failed to create ImportSource for AssetImporter")?;
    let _asset_importer = AssetImporter::new(import_source, 4).wrap_err("Failed to create AssetImporter")?;

    {
        let cameras = renderer.cameras();
        let handle = renderer.active_camera(window1.id()).wrap_err("Failed to get active camera")?;
        let camera = cameras.get(&handle).ok_or(eyre!("Failed to get camera"))?;
        println!("Camera: {:?}", camera.matrix());
    }

    let model = load_model().wrap_err("Failed to load model")?;

    let inanimate_mesh1 = renderer
        .inanimate_meshes()
        .create(MeshType::TriangleList, model)
        .with_debug_info(debug_info!("my_mesh"))
        .build()
        .wrap_err("Failed to create inanimate mesh")?;

    {
        let mut inanimate_mesh_instance = renderer.inanimate_mesh_instances();
        inanimate_mesh_instance
            .insert(InanimateMeshInstance::new(inanimate_mesh1.clone(), Affine3::identity()))
            .wrap_err("Failed to insert inanimate mesh instance")?;
    }

    let mut loop_helper = spin_sleep::LoopHelper::builder().build_with_target_rate(60.0);
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window1.id() => control_flow.set_exit(),
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(..),
            } => {
                if let Err(err) = renderer.window_resized(window_id) {
                    error!("Failed to resize window: {}", err);
                    control_flow.set_exit();
                }
            }
            Event::MainEventsCleared => {
                loop_helper.loop_start();

                if let Err(err) = immediate_rendering(&renderer) {
                    error!("Failed to do immediate rendering: {}", err);
                    control_flow.set_exit();
                    return;
                }

                if let Err(err) = renderer.render_frame() {
                    error!("Failed to render frame: {}", err);
                    control_flow.set_exit();
                    return;
                }

                // loop_helper.loop_sleep();
            }
            _ => (),
        }
    });
}
