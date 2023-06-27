use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use jeriya_backend_ash_core::{command_pool::CommandPool, device::Device, queue::Queue};
use jeriya_shared::{
    derive_more::Constructor, parking_lot::Mutex, winit::window::WindowId, Camera, CameraEvent, EventQueue, IndexingContainer,
};

use crate::ImmediateRenderingRequest;

/// Elements of the backend that are shared between all [`Presenter`]s.
#[derive(Constructor)]
pub struct AshSharedBackend {
    pub device: Arc<Device>,
    pub presentation_queue: RefCell<Queue>,
    pub command_pool: Rc<CommandPool>,
    pub immediate_rendering_requests: Mutex<HashMap<WindowId, Vec<ImmediateRenderingRequest>>>,
    pub cameras: Arc<Mutex<IndexingContainer<Camera>>>,
    pub camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,
}
