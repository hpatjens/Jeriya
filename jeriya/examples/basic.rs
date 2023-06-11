use std::io;

use jeriya_backend_ash::AshBackend;
use jeriya_shared::{
    debug_info,
    immediate::{LineConfig, LineList, TriangleConfig, TriangleList, TriangleStrip},
    log,
    nalgebra::Vector3,
    winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        window::WindowBuilder,
    },
};

fn main() -> io::Result<()> {
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
    let window = WindowBuilder::new()
        .with_title("Example")
        .with_inner_size(jeriya_shared::winit::dpi::LogicalSize::new(640.0, 480.0))
        .build(&event_loop)
        .unwrap();

    let renderer = jeriya::Renderer::<AshBackend>::builder().add_windows(&[&window]).build().unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(..),
            } => {
                renderer.window_resized(window_id).unwrap();
            }
            Event::MainEventsCleared => {
                window.request_redraw();

                let immediate_command_buffer_builder = renderer
                    .create_immediate_command_buffer_builder(debug_info!("my_command_buffer"))
                    .unwrap();

                let line_list = LineList::new(
                    vec![Vector3::new(-0.5, -0.5, 0.0), Vector3::new(1.0, 1.0, 0.0)],
                    LineConfig::default(),
                );
                let triangle_list = TriangleList::new(
                    vec![
                        Vector3::new(-0.8, -0.8, 0.0),
                        Vector3::new(-0.8, -0.6, 0.0),
                        Vector3::new(-0.5, -0.7, 0.0),
                        Vector3::new(-0.5, -0.7, 0.0),
                        Vector3::new(-0.5, -0.5, 0.0),
                        Vector3::new(-0.2, -0.6, 0.0),
                    ],
                    TriangleConfig::default(),
                );
                let triangle_strip = TriangleStrip::new(
                    vec![
                        Vector3::new(0.7, -0.8, 0.0),
                        Vector3::new(0.3, -0.8, 0.0),
                        Vector3::new(0.7, -0.6, 0.0),
                        Vector3::new(0.3, -0.5, 0.0),
                    ],
                    TriangleConfig::default(),
                );

                let immediate_command_buffer = immediate_command_buffer_builder
                    .push_line_lists(&[line_list])
                    .unwrap()
                    .push_triangle_lists(&[triangle_list])
                    .unwrap()
                    .push_triangle_strips(&[triangle_strip])
                    .unwrap()
                    .build()
                    .unwrap();

                renderer.render_immediate_command_buffer(immediate_command_buffer).unwrap();

                renderer.render_frame().unwrap();
            }
            _ => (),
        }
    });
}
