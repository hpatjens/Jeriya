use std::io;

use jeriya_backend_ash::Ash;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

fn main() -> io::Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
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
        .with_inner_size(winit::dpi::LogicalSize::new(640.0, 480.0))
        .build(&event_loop)
        .unwrap();

    let renderer = jeriya::Renderer::<Ash>::builder().add_windows(&[&window]).build().unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            Event::MainEventsCleared => {
                window.request_redraw();

                renderer.render_frame();
            }
            _ => (),
        }
    });
}
