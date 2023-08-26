mod immediate_command_buffer {
    // use jeriya_backend_ash::AshBackend;
    // use jeriya_shared::{
    //     debug_info,
    //     immediate::{CommandBufferConfig, Line},
    //     nalgebra::Vector3,
    // };
    // use jeriya_test::create_window;

    #[test]
    fn smoke() -> jeriya_backend::Result<()> {
        // let window = create_window();
        // let renderer = jeriya::Renderer::<AshBackend>::builder().add_windows(&[&window]).build().unwrap();
        // renderer
        //     .create_command_buffer_builder(debug_info!("my_command_buffer"))?
        //     .push_line(Line::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 0.0)))?
        //     .set_config(CommandBufferConfig::default())?
        //     .build()?;
        Ok(())
    }
}
