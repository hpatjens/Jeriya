use std::sync::Arc;

use base::queue_plan::QueueSelection;
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{device::Device, queue::Queue};
use jeriya_shared::parking_lot::MutexGuard;
use jeriya_shared::{debug_info, parking_lot::Mutex, winit::window::WindowId};

pub struct Queues {
    presentation_queue: Queue,
}

impl Queues {
    /// Returns the queue that should be used for presentation operations on the given window.
    pub fn presentation_queue(&mut self, _window_id: WindowId) -> &mut Queue {
        &mut self.presentation_queue
    }

    /// Returns the queue that should be used for transfer operations.
    pub fn transfer_queue(&mut self) -> &mut Queue {
        &mut self.presentation_queue
    }
}

pub struct QueueScheduler {
    queues: Mutex<Queues>,
}

impl QueueScheduler {
    pub fn new(device: &Arc<Device>) -> base::Result<Self> {
        let queues = Queues {
            presentation_queue: Queue::new(&device, &QueueSelection::new_unchecked(0, 0), debug_info!("main-queue"))?,
        };
        Ok(Self {
            queues: Mutex::new(queues),
        })
    }

    /// Returns the queues that were created on the device.
    ///
    /// All queues are behind a mutex so that they can be locked all at once. Locking only
    /// individual queues would be useful for individual submissions but for swapchain
    /// recreation, all queues need to be locked at once.
    pub fn queues(&self) -> MutexGuard<Queues> {
        self.queues.lock()
    }
}
