use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use jeriya_backend_ash_base::{
    command_pool::{CommandPool, CommandPoolCreateFlags},
    device::Device,
    queue::{Queue, QueueType},
};
use jeriya_shared::{
    debug_info, log::info, parking_lot::Mutex, winit::window::WindowId, Camera, CameraEvent, EventQueue, IndexingContainer, RendererConfig,
};

use crate::ImmediateRenderingRequest;

/// Elements of the backend that are shared between all [`Presenter`]s.
pub struct BackendShared {
    pub device: Arc<Device>,
    pub renderer_config: Arc<RendererConfig>,
    pub presentation_queue: RefCell<Queue>,
    pub command_pool: Rc<CommandPool>,
    pub immediate_rendering_requests: Mutex<HashMap<WindowId, Vec<ImmediateRenderingRequest>>>,
    pub cameras: Arc<Mutex<IndexingContainer<Camera>>>,
    pub camera_event_queue: Arc<Mutex<EventQueue<CameraEvent>>>,
}

impl BackendShared {
    pub fn new(device: &Arc<Device>, renderer_config: &Arc<RendererConfig>) -> jeriya_shared::Result<Self> {
        info!("Creating Cameras");
        let cameras = Arc::new(Mutex::new(IndexingContainer::new()));
        let camera_event_queue = Arc::new(Mutex::new(EventQueue::new()));

        // Presentation Queue
        let presentation_queue = Queue::new(device, QueueType::Presentation)?;

        info!("Creating CommandPool");
        let command_pool = CommandPool::new(
            device,
            &presentation_queue,
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;

        Ok(Self {
            device: device.clone(),
            renderer_config: renderer_config.clone(),
            presentation_queue: RefCell::new(presentation_queue),
            command_pool,
            immediate_rendering_requests: Mutex::new(HashMap::new()),
            cameras,
            camera_event_queue,
        })
    }
}
