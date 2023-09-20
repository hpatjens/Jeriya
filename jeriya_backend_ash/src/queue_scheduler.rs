use std::collections::BTreeMap;
use std::sync::Arc;

use base::queue_plan::QueueSelection;
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{device::Device, queue::Queue};
use jeriya_shared::log::info;
use jeriya_shared::parking_lot::MutexGuard;
use jeriya_shared::{debug_info, parking_lot::Mutex, winit::window::WindowId};

pub struct Queues {
    presentation_queues: Vec<Queue>,
    presentation_queue_mapping: BTreeMap<WindowId, usize>,

    transfer_queue: Queue,
}

impl Queues {
    /// Returns the queue that should be used for presentation operations on the given window.
    pub fn presentation_queue(&mut self, window_id: WindowId) -> &mut Queue {
        self.presentation_queue_mapping
            .get(&window_id)
            .and_then(|index| self.presentation_queues.get_mut(*index))
            .expect("No presentation queue for window")
    }

    /// Returns the queue that should be used for transfer operations.
    pub fn transfer_queue(&mut self) -> &mut Queue {
        &mut self.transfer_queue
    }
}

pub struct QueueScheduler {
    queues: Mutex<Queues>,
}

impl QueueScheduler {
    pub fn new(device: &Arc<Device>) -> base::Result<Self> {
        let presentation_queues = device
            .queue_plan
            .presentation_queues
            .iter()
            .map(|queue_selection| {
                let name = format! {
                    "Presentation-Queue-Family-Index-{}-Queue-Index-{}",
                    queue_selection.queue_family_index(),
                    queue_selection.queue_index()
                };
                info!("Getting Presentation Queue: {}", name);
                Queue::new(device, queue_selection, debug_info!(name))
            })
            .collect::<Result<Vec<Queue>, _>>()?;
        let presentation_queue_mapping = device.queue_plan.presentation_queue_mapping.clone();

        info!("Getting Transfer Queue");
        let transfer_queue = Queue::new(device, &device.queue_plan.transfer_queue, debug_info!("Transfer-Queue"))?;

        let queues = Queues {
            presentation_queues,
            presentation_queue_mapping,
            transfer_queue,
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
