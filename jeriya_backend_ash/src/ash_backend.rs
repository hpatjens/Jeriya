use std::{
    cell::RefCell,
    collections::HashMap,
    mem,
    sync::{mpsc::Receiver, Arc},
};

use crate::{
    ash_immediate::{AshImmediateCommandBufferBuilderHandler, AshImmediateCommandBufferHandler},
    backend_shared::BackendShared,
    presenter::Presenter,
};
use base::{command_buffer::CommandBuffer, command_buffer_builder::CommandBufferBuilder, shader_interface};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    debug::{set_panic_on_message, ValidationLayerCallback},
    device::Device,
    entry::Entry,
    instance::Instance,
    physical_device::PhysicalDevice,
    surface::Surface,
    Config, ValidationLayerConfig,
};
use jeriya_shared::{
    debug_info, immediate,
    inanimate_mesh::{InanimateMeshEvent, InanimateMeshGpuState, InanimateMeshGroup},
    log::{info, warn},
    winit::window::{Window, WindowId},
    AsDebugInfo, Backend, Camera, CameraContainerGuard, DebugInfo, Handle, ImmediateCommandBufferBuilderHandler,
    InanimateMeshInstanceContainerGuard, RendererConfig,
};

#[derive(Debug)]
pub struct ImmediateRenderingRequest {
    pub immediate_command_buffer: AshImmediateCommandBufferHandler,
    pub count: usize,
}

pub struct AshBackend {
    presenters: HashMap<WindowId, RefCell<Presenter>>,
    _surfaces: HashMap<WindowId, Arc<Surface>>,
    _validation_layer_callback: Option<ValidationLayerCallback>,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
    backend_shared: BackendShared,
}

impl Backend for AshBackend {
    type BackendConfig = Config;

    type ImmediateCommandBufferBuilderHandler = AshImmediateCommandBufferBuilderHandler;
    type ImmediateCommandBufferHandler = AshImmediateCommandBufferHandler;

    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> jeriya_shared::Result<Self>
    where
        Self: Sized,
    {
        if windows.is_empty() {
            return Err(jeriya_shared::Error::ExpectedWindow);
        }

        info!("Creating Vulkan Entry");
        let entry = Entry::new()?;

        info!("Creating Vulkan Instance");
        let application_name = renderer_config
            .application_name
            .clone()
            .unwrap_or(env!("CARGO_PKG_NAME").to_owned());
        let instance = Instance::new(
            &entry,
            &application_name,
            matches!(backend_config.validation_layer, ValidationLayerConfig::Enabled { .. }),
        )?;

        let validation_layer_callback = match backend_config.validation_layer {
            ValidationLayerConfig::Disabled => {
                info!("Skipping validation layer callback setup");
                None
            }
            ValidationLayerConfig::Enabled { panic_on_message } => {
                info!("Setting up validation layer callback");
                set_panic_on_message(panic_on_message);
                Some(ValidationLayerCallback::new(&entry, &instance)?)
            }
        };

        let windows = windows.iter().map(|window| (window.id(), window)).collect::<HashMap<_, _>>();
        let surfaces = windows
            .iter()
            .map(|(window_id, window)| {
                info!("Creating Surface for window {window_id:?}");
                let surface = Surface::new(&entry, &instance, window)?;
                Ok((*window_id, surface))
            })
            .collect::<base::Result<HashMap<WindowId, Arc<Surface>>>>()?;

        info!("Creating PhysicalDevice");
        let physical_device = PhysicalDevice::new(&instance, surfaces.values())?;

        info!("Creating Device");
        let device = Device::new(physical_device, &instance)?;

        let backend_shared = BackendShared::new(&device, &Arc::new(renderer_config))?;

        let presenters = surfaces
            .iter()
            .map(|(window_id, surface)| {
                info!("Creating presenter for window {window_id:?}");
                let presenter = Presenter::new(window_id, surface, &backend_shared)?;
                Ok((*window_id, RefCell::new(presenter)))
            })
            .collect::<jeriya_shared::Result<HashMap<_, _>>>()?;

        Ok(Self {
            _entry: entry,
            _instance: instance,
            _surfaces: surfaces,
            _validation_layer_callback: validation_layer_callback,
            presenters,
            backend_shared,
        })
    }

    fn handle_window_resized(&self, window_id: WindowId) -> jeriya_shared::Result<()> {
        let mut presenter = self
            .presenters
            .get(&window_id)
            .ok_or_else(|| base::Error::UnknownWindowId(window_id))?
            .borrow_mut();
        presenter.recreate()?;
        Ok(())
    }

    fn handle_render_frame(&self) -> jeriya_shared::Result<()> {
        self.backend_shared.presentation_queue.borrow_mut().poll_completed_fences()?;

        if !self.backend_shared.inanimate_mesh_event_queue.lock().is_empty() {
            // Create a new command buffer for maintaining the meshes
            let mut command_buffer = CommandBuffer::new(
                &self.backend_shared.device,
                &self.backend_shared.command_pool.clone(),
                debug_info!("maintanance-CommandBuffer"),
            )?;
            let mut command_buffer_builder = CommandBufferBuilder::new(&self.backend_shared.device, &mut command_buffer)?;
            command_buffer_builder.begin_command_buffer_for_one_time_submit()?;

            // Handle inanimate mesh events
            while let Some(event) = self.backend_shared.inanimate_mesh_event_queue.lock().pop() {
                match event {
                    InanimateMeshEvent::Insert {
                        inanimate_mesh,
                        vertex_positions,
                        indices,
                    } => {
                        let vertices_start_offset = self
                            .backend_shared
                            .static_vertex_buffer
                            .lock()
                            .push(&vertex_positions, &mut command_buffer_builder)?;
                        let inanimate_mesh_gpu = shader_interface::InanimateMesh {
                            start_offset: vertices_start_offset as u64,
                            vertices_len: vertex_positions.len() as u64,
                        };
                        info!(
                            "Inserting a new inanimate mesh with start_offset: {start_offset} and vertices_len: {vertices_len}",
                            start_offset = vertices_start_offset,
                            vertices_len = vertex_positions.len()
                        );
                        let inanimate_mesh_start_offset = self
                            .backend_shared
                            .inanimate_mesh_buffer
                            .lock()
                            .push(&[inanimate_mesh_gpu], &mut command_buffer_builder)?;
                        let inanimate_mesh2 = inanimate_mesh.clone();
                        let inanimate_mesh_gpu_states2 = self.backend_shared.inanimate_mesh_gpu_states.clone();

                        // Insert the GPU state for the InanimateMesh when the upload to the GPU is done
                        command_buffer_builder.push_finished_operation(Box::new(move || {
                            let handle = inanimate_mesh2.handle();
                            inanimate_mesh_gpu_states2.lock().insert(
                                handle.clone(),
                                InanimateMeshGpuState::Uploaded {
                                    inanimate_mesh_offset: inanimate_mesh_start_offset as u64,
                                },
                            );
                            info!(
                                "Upload of inanimate mesh {} ({:?}) to GPU is done",
                                inanimate_mesh.as_debug_info().format_one_line(),
                                handle
                            );
                            Ok(())
                        }));
                    }
                    InanimateMeshEvent::SetVertexPositions {
                        inanimate_mesh,
                        vertex_posisions,
                    } => {
                        todo!()
                    }
                }
            }
            command_buffer_builder.end_command_buffer()?;
            self.backend_shared.presentation_queue.borrow_mut().submit(command_buffer)?;
        }

        // Render on all surfaces
        for (window_id, presenter) in &self.presenters {
            let presenter = &mut *presenter.borrow_mut();
            presenter.render_frame(window_id, &self.backend_shared)?;
        }

        Ok(())
    }

    fn create_immediate_command_buffer_builder(
        &self,
        debug_info: DebugInfo,
    ) -> jeriya_shared::Result<immediate::CommandBufferBuilder<Self>> {
        let command_buffer_builder = AshImmediateCommandBufferBuilderHandler::new(self, debug_info)?;
        Ok(immediate::CommandBufferBuilder::new(command_buffer_builder))
    }

    fn render_immediate_command_buffer(&self, command_buffer: Arc<immediate::CommandBuffer<Self>>) -> jeriya_shared::Result<()> {
        for presenter in self.presenters.values() {
            let immediate_rendering_request = ImmediateRenderingRequest {
                immediate_command_buffer: AshImmediateCommandBufferHandler {
                    commands: command_buffer.command_buffer().commands.clone(),
                    debug_info: command_buffer.command_buffer().debug_info.clone(),
                },
                count: 1,
            };
            presenter.borrow_mut().render_immediate_command_buffer(immediate_rendering_request);
        }
        Ok(())
    }

    fn cameras(&self) -> CameraContainerGuard {
        CameraContainerGuard::new(
            self.backend_shared.camera_event_queue.lock(),
            self.backend_shared.cameras.lock(),
            self.backend_shared.renderer_config.clone(),
        )
    }

    fn inanimate_meshes(&self) -> &InanimateMeshGroup {
        &self.backend_shared.inanimate_meshes
    }

    fn inanimate_mesh_instances(&self) -> InanimateMeshInstanceContainerGuard {
        InanimateMeshInstanceContainerGuard::new(
            self.backend_shared.inanimate_mesh_instance_event_queue.lock(),
            self.backend_shared.inanimate_mesh_instances.lock(),
            self.backend_shared.renderer_config.clone(),
        )
    }

    fn set_active_camera(&self, window_id: WindowId, handle: Handle<Camera>) -> jeriya_shared::Result<()> {
        let presenter = self
            .presenters
            .get(&window_id)
            .ok_or(jeriya_shared::Error::UnknownWindowId(window_id))?;
        presenter.borrow_mut().set_active_camera(handle);
        Ok(())
    }

    fn active_camera(&self, window_id: WindowId) -> jeriya_shared::Result<Handle<Camera>> {
        self.presenters
            .get(&window_id)
            .ok_or(jeriya_shared::Error::UnknownWindowId(window_id))
            .map(|presenter| presenter.borrow().active_camera())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod backend_new {
        use jeriya_test::create_window;

        use super::*;

        #[test]
        fn smoke() {
            let window = create_window();
            let renderer_config = RendererConfig {
                application_name: Some("my_application".to_owned()),
                ..RendererConfig::default()
            };
            let backend_config = Config::default();
            AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn application_name_none() {
            let window = create_window();
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
        }

        #[test]
        fn empty_windows_none() {
            let renderer_config = RendererConfig::default();
            let backend_config = Config::default();
            assert!(matches!(
                AshBackend::new(renderer_config, backend_config, &[]),
                Err(jeriya_shared::Error::ExpectedWindow)
            ));
        }
    }

    mod render_frame {
        use jeriya_test::create_window;

        use super::*;

        #[test]
        fn smoke() {
            let window = create_window();
            let renderer_config = RendererConfig {
                application_name: Some("my_application".to_owned()),
                ..RendererConfig::default()
            };
            let backend_config = Config::default();
            let backend = AshBackend::new(renderer_config, backend_config, &[&window]).unwrap();
            backend.handle_render_frame().unwrap();
        }
    }
}
