use std::{collections::BTreeMap, iter, sync::Arc};

use ash::vk::{self, QueueFamilyProperties, QueueFlags};
use jeriya_shared::{
    itertools::Itertools,
    log::{info, log_enabled, Level},
    winit::window::WindowId,
};

use crate::{instance::Instance, physical_device::PhysicalDevice, surface::Surface, AsRawVulkan, Error};

/// Identifies a queue that should be created
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueSelection {
    queue_family_index: u32,
    queue_index: u32,
}

impl QueueSelection {
    /// Creates a new `QueueSelection` for the given queue family index and queue index
    ///
    /// The caller is responsible for ensuring that the queue family index and queue index are valid.
    pub fn new_unchecked(queue_family_index: u32, queue_index: u32) -> Self {
        Self {
            queue_family_index,
            queue_index,
        }
    }

    /// Returns the queue family index
    pub fn queue_family_index(&self) -> u32 {
        self.queue_family_index
    }

    /// Returns the queue index
    pub fn queue_index(&self) -> u32 {
        self.queue_index
    }
}

/// The plan for creating queues
#[derive(Debug, Clone)]
pub struct QueuePlan {
    /// The presentation queues that should be created according to the plan. When possible there is a queue for each surface.
    pub presentation_queues: Vec<QueueSelection>,
    /// Mapping from window id to the index of the presentation queue
    pub presentation_queue_mapping: BTreeMap<WindowId, usize>,

    /// The transfer queue that should be created according to the plan
    pub transfer_queue: QueueSelection,

    /// The queue family indices that are used by the queues
    pub queue_family_indices: Vec<u32>,
}

impl QueuePlan {
    /// Creates a new `QueuePlan` for the given `PhysicalDevice` and `Surface`s.
    pub fn new<'w, 's>(
        instance: &Instance,
        physical_device: &PhysicalDevice,
        surfaces: impl IntoIterator<Item = (&'w WindowId, &'s Arc<Surface>)>,
    ) -> crate::Result<Self> {
        let surfaces = surfaces.into_iter().collect::<Vec<_>>();

        let queue_family_properties = unsafe {
            instance
                .as_raw_vulkan()
                .get_physical_device_queue_family_properties(*physical_device.as_raw_vulkan())
        };

        // Create a map containing the support for each surface and queue family
        let mut surface_support = BTreeMap::new();
        for (queue_family_index, _) in queue_family_properties.iter().enumerate() {
            for (window_id, surface) in &surfaces {
                let support = surface.supports_presentation(physical_device.as_raw_vulkan(), queue_family_index)?;
                surface_support.insert((**window_id, queue_family_index as u32), support);
            }
        }

        // Print a table that summarizes the surface support
        if log_enabled!(Level::Info) {
            let mut message = String::new();
            let header = format! {
                "| {:<30} | {:<20} | {:<60} | {:<16} |\n",
                "WindowId", "QueueFamilyIndex", "Flags", "SurfaceSupport"
            };
            message.push_str(&header);
            for ((window_id, queue_family_index), surface_support) in &surface_support {
                let queue_family_properties = queue_family_properties
                    .get(*queue_family_index as usize)
                    .expect("failed to find queue family properties");
                let window_id = format!("{:?}", window_id);
                let flags = format!("{:?}", queue_family_properties.queue_flags);
                let line = format! {
                    "| {:<30} | {:<20} | {:<60} | {:<16} |\n",
                    window_id, queue_family_index, flags, surface_support
                };
                message.push_str(&line);
            }
            info!("Surface support:\n{message}");
        }

        let window_ids = surfaces.iter().map(|(window_id, _)| *window_id);
        plan_queues(window_ids, queue_family_properties, &surface_support)
    }

    /// Returns an iterator over all queues in the `QueuePlan`
    pub fn iter_queue_selections(&self) -> impl Iterator<Item = &QueueSelection> {
        self.presentation_queues.iter().chain(iter::once(&self.transfer_queue))
    }
}

fn plan_queues<'s>(
    surfaces: impl Iterator<Item = &'s WindowId>,
    queue_family_properties: Vec<QueueFamilyProperties>,
    surface_support: &BTreeMap<(WindowId, u32), bool>,
) -> Result<QueuePlan, Error> {
    // Create a map for keeping track of which queue is assigned to which surface
    let mut all_presentation_queues = BTreeMap::<WindowId, QueueSelection>::new();

    // Select the presentation queues
    for window_id in surfaces {
        if let Some(presentation_queue) =
            find_unshared_presentation_queue(&queue_family_properties, &window_id, surface_support, &all_presentation_queues)
                .or(find_least_stressed_presentation_queue(
                    &queue_family_properties,
                    &window_id,
                    &surface_support,
                    &all_presentation_queues,
                ))
                .or(find_any_presentation_queue(&queue_family_properties, &window_id, &surface_support))
        {
            all_presentation_queues.insert(*window_id, presentation_queue);
        }
    }

    // Select the transfer queue
    let transfer_queue = find_unshared_transfer_queue(&queue_family_properties, &mut all_presentation_queues)?
        .or(find_any_dedicated_transfer_queue(&queue_family_properties))
        .or(find_any_transfer_queue(&queue_family_properties))
        .ok_or(Error::NoSuitableQueues)?;

    // Collapse the presentation queues so that a `QueueSelection` is only emitted once
    let mut presentation_queues = Vec::new();
    let mut presentation_queue_mapping = BTreeMap::new();
    for (window_id, queue_selection) in &all_presentation_queues {
        if let Some(index) = presentation_queues.iter().position(|selection| selection == queue_selection) {
            presentation_queue_mapping.insert(window_id.clone(), index);
        } else {
            let index = presentation_queues.len();
            presentation_queues.push(queue_selection.clone());
            presentation_queue_mapping.insert(window_id.clone(), index);
        }
    }

    // Collect all queue families that are used by the `QueuePlan`
    let queue_family_indices = presentation_queues
        .iter()
        .map(|selection| selection.queue_family_index)
        .chain(iter::once(transfer_queue.queue_family_index))
        .unique()
        .collect();

    Ok(QueuePlan {
        presentation_queue_mapping,
        presentation_queues,
        transfer_queue,
        queue_family_indices,
    })
}

/// Find a queue that can be used for presentation
fn find_any_presentation_queue(
    queue_family_properties: &Vec<QueueFamilyProperties>,
    window_id: &WindowId,
    surface_support: &BTreeMap<(WindowId, u32), bool>,
) -> Option<QueueSelection> {
    for (queue_family_index, _) in queue_family_properties.iter().enumerate() {
        if *surface_support
            .get(&(*window_id, queue_family_index as u32))
            .expect("failed to find surface support")
        {
            return Some(QueueSelection {
                queue_family_index: queue_family_index as u32,
                queue_index: 0,
            });
        }
    }
    None
}

/// Find a queue that is not already assigned to a surface.
fn find_unshared_presentation_queue(
    queue_family_properties: &Vec<QueueFamilyProperties>,
    window_id: &WindowId,
    surface_support: &BTreeMap<(WindowId, u32), bool>,
    presentation_queues: &BTreeMap<WindowId, QueueSelection>,
) -> Option<QueueSelection> {
    for (queue_family_index, queue_family_properties) in queue_family_properties.iter().enumerate() {
        if *surface_support
            .get(&(*window_id, queue_family_index as u32))
            .expect("failed to find surface support")
        {
            let queue_index = find_next_free_queue_index(presentation_queues, queue_family_index as u32, queue_family_properties);
            if let Some(queue_index) = queue_index {
                return Some(QueueSelection {
                    queue_family_index: queue_family_index as u32,
                    queue_index,
                });
            }
        }
    }
    None
}

/// Find the queue that is assigned to the least number of surfaces.
fn find_least_stressed_presentation_queue(
    queue_family_properties: &Vec<QueueFamilyProperties>,
    window_id: &WindowId,
    surface_support: &BTreeMap<(WindowId, u32), bool>,
    presentation_queues: &BTreeMap<WindowId, QueueSelection>,
) -> Option<QueueSelection> {
    let mut sorted_queue_family_properties = queue_family_properties
        .iter()
        .enumerate()
        // We are only interested in the queue families that support presentation
        .filter(|(queue_family_index, _properties)| {
            *surface_support
                .get(&(*window_id, *queue_family_index as u32))
                .expect("failed to find surface support")
        })
        // Create a QueueSelection for every queue in the queue families
        .flat_map(|(queue_family_index, properties)| {
            (0..properties.queue_count).map(move |queue_index| QueueSelection {
                queue_family_index: queue_family_index as u32,
                queue_index,
            })
        })
        .collect::<Vec<_>>();

    // Sort the queue families by the number of surfaces that are assigned to them
    sorted_queue_family_properties.sort_by_key(|queue_selection| {
        presentation_queues
            .values()
            .filter(|selection| *selection == queue_selection)
            .count()
    });

    sorted_queue_family_properties.iter().next().cloned()
}

/// Find the queue index that is not already assigned in the queue family
fn find_next_free_queue_index(
    presentation_queues: &BTreeMap<WindowId, QueueSelection>,
    queue_family_index: u32,
    queue_family_properties: &QueueFamilyProperties,
) -> Option<u32> {
    let max_queue_index = presentation_queues
        .values()
        .filter(|selection| selection.queue_family_index == queue_family_index)
        .map(|selection| selection.queue_index)
        .max();
    if let Some(max_queue_index) = max_queue_index {
        let next_queue_index = max_queue_index + 1;
        if next_queue_index < queue_family_properties.queue_count {
            // There is a free queue in the queue family
            Some(next_queue_index)
        } else {
            None
        }
    } else {
        Some(0)
    }
}

/// Tests whether the queue family represents the transfer queues
fn is_dedicated_transfer_queue_family(queue_family_properties: &QueueFamilyProperties) -> bool {
    queue_family_properties.queue_flags.contains(vk::QueueFlags::TRANSFER)
        && !queue_family_properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        && !queue_family_properties.queue_flags.contains(vk::QueueFlags::COMPUTE)
}

/// Returns the number of capabilities that are supported by the queue family
fn capability_count(queue_flags: QueueFlags) -> usize {
    let mut count = 0;
    if queue_flags.contains(QueueFlags::GRAPHICS) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::COMPUTE) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::TRANSFER) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::SPARSE_BINDING) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::VIDEO_DECODE_KHR) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::VIDEO_ENCODE_KHR) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::PROTECTED) {
        count += 1;
    }
    if queue_flags.contains(QueueFlags::OPTICAL_FLOW_NV) {
        count += 1;
    }
    count
}

/// Sort the queue families by the number of capabilities that they support
fn sort_by_capability_count(queue_family_properties: &Vec<QueueFamilyProperties>) -> Vec<(u32, QueueFamilyProperties)> {
    let mut queue_family_properties = queue_family_properties
        .iter()
        .enumerate()
        .map(|(index, properties)| (index as u32, *properties))
        .collect::<Vec<_>>();
    queue_family_properties.sort_by_key(|(_index, properties)| capability_count(properties.queue_flags));
    queue_family_properties
}

/// Find a queue that is not already assigned to a surface and is either a dedicated transfer queue or supports vk::QueueFlags::TRANSFER.
fn find_unshared_transfer_queue(
    queue_family_properties: &Vec<QueueFamilyProperties>,
    presentation_queues: &BTreeMap<WindowId, QueueSelection>,
) -> crate::Result<Option<QueueSelection>> {
    // Frist, we try to find a queue that is a dedicated transfer queue
    for (queue_family_index, queue_family_properties) in queue_family_properties.iter().enumerate() {
        if is_dedicated_transfer_queue_family(queue_family_properties) {
            let queue_index = find_next_free_queue_index(presentation_queues, queue_family_index as u32, queue_family_properties);
            if let Some(queue_index) = queue_index {
                return Ok(Some(QueueSelection {
                    queue_family_index: queue_family_index as u32,
                    queue_index,
                }));
            }
        }
    }
    // Second, we try to find a queue that supports vk::QueueFlags::TRANSFER but has the smallest number of capabilities
    let least_capable_families = sort_by_capability_count(queue_family_properties);
    for (queue_family_index, queue_family_properties) in &least_capable_families {
        if queue_family_properties.queue_flags.contains(vk::QueueFlags::TRANSFER) {
            let queue_index = find_next_free_queue_index(presentation_queues, *queue_family_index, queue_family_properties);
            if let Some(queue_index) = queue_index {
                return Ok(Some(QueueSelection {
                    queue_family_index: *queue_family_index,
                    queue_index,
                }));
            }
        }
    }
    Ok(None)
}

/// Find a queue that should be used for transfer because it doesn't support vk::QueueFlags::GRAPHICS or vk::QueueFlags::COMPUTE.
fn find_any_dedicated_transfer_queue(queue_family_properties: &Vec<QueueFamilyProperties>) -> Option<QueueSelection> {
    for (queue_family_index, queue_family_properties) in queue_family_properties.iter().enumerate() {
        if is_dedicated_transfer_queue_family(queue_family_properties) {
            return Some(QueueSelection {
                queue_family_index: queue_family_index as u32,
                queue_index: 0,
            });
        }
    }
    None
}

/// Find any queue that supports vk::QueueFlags::TRANSFER without considering other flags.
fn find_any_transfer_queue(queue_family_properties: &Vec<QueueFamilyProperties>) -> Option<QueueSelection> {
    for (queue_family_index, queue_family_properties) in queue_family_properties.iter().enumerate() {
        if queue_family_properties.queue_flags.contains(vk::QueueFlags::TRANSFER) {
            return Some(QueueSelection {
                queue_family_index: queue_family_index as u32,
                queue_index: 0,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use ash::vk::QueueFlags;
    use jeriya_shared::maplit::btreemap;

    use super::*;

    macro_rules! queue_family_properties {
        ($($count:literal for $flags:expr),*) => {
            vec![
                $(
                    QueueFamilyProperties {
                        queue_flags: $flags,
                        queue_count: $count,
                        ..Default::default()
                    },
                )*
            ]
        }
    }

    macro_rules! assert_queue_selection {
        ($queue_selection:expr; family $queue_family_index:expr, $queue_index:expr) => {
            assert_eq!(
                $queue_selection,
                QueueSelection {
                    queue_family_index: $queue_family_index,
                    queue_index: $queue_index
                }
            );
        };
    }

    mod find_any_dedicated_transfer_queue {
        use super::*;

        #[test]
        fn success() {
            let queue_family_properties = queue_family_properties! {
                8 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                2 for QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER
            };

            let transfer_queue = find_any_dedicated_transfer_queue(&queue_family_properties).unwrap();
            assert_queue_selection!(transfer_queue; family 1, 0);
        }

        #[test]
        fn failure() {
            let queue_family_properties = queue_family_properties! {
                1 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING
            };
            let transfer_queue = find_any_dedicated_transfer_queue(&queue_family_properties);
            assert!(transfer_queue.is_none());
        }
    }

    mod find_any_transfer_queue {
        use super::*;

        #[test]
        fn success() {
            let queue_family_properties = queue_family_properties! {
                2 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING
            };

            let transfer_queue = find_any_transfer_queue(&queue_family_properties).unwrap();
            assert_queue_selection!(transfer_queue; family 0, 0);
        }

        #[test]
        fn failure() {
            let queue_family_properties = queue_family_properties! {
                1 for QueueFlags::COMPUTE
            };

            let transfer_queue = find_any_dedicated_transfer_queue(&queue_family_properties);
            assert!(transfer_queue.is_none());
        }
    }

    mod find_unshared_transfer_queue {
        use super::*;

        #[test]
        fn success() {
            let queue_family_properties = queue_family_properties! {
                8 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER
            };

            let presentation_queues = btreemap! {
                WindowId::from(0) => QueueSelection {
                    queue_family_index: 0,
                    queue_index: 0
                },
                WindowId::from(1) => QueueSelection {
                    queue_family_index: 0,
                    queue_index: 1
                },
            };

            let transfer_queue = find_unshared_transfer_queue(&queue_family_properties, &presentation_queues)
                .unwrap()
                .unwrap();
            assert_queue_selection!(transfer_queue; family 1, 0);
        }
    }

    mod sort_by_capability_count {
        use super::*;

        #[test]
        fn smoke() {
            let queue_family_properties = queue_family_properties! {
                /* 0 */ 8 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                /* 1 */ 2 for QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                /* 2 */ 4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER,
                /* 3 */ 1 for QueueFlags::COMPUTE,
                /* 4 */ 2 for QueueFlags::TRANSFER | QueueFlags::GRAPHICS
            };
            let result = sort_by_capability_count(&queue_family_properties);
            let queue_family_indices = result.iter().map(|(index, _)| *index).collect::<Vec<_>>();
            assert_eq!(result.len(), 5);
            assert_eq!(queue_family_indices, vec![3, 1, 4, 2, 0]);
        }
    }

    mod find_unshared_presentation_queue {
        use super::*;

        /// Tests the `find_unshared_presentation_queue` function with the given parameters.
        fn test_fn(
            queue_family_properties: &Vec<QueueFamilyProperties>,
            window_id: &WindowId,
            surface_support: &BTreeMap<(WindowId, u32), bool>,
            allpresentation_queues: &mut BTreeMap<WindowId, QueueSelection>,
            expected_queue_family_index: u32,
            expected_queue_index: u32,
        ) {
            let queue =
                find_unshared_presentation_queue(&queue_family_properties, &window_id, &surface_support, &allpresentation_queues).unwrap();

            // Store the queue so that is counts as assigned
            allpresentation_queues.insert(*window_id, queue.clone());

            assert_queue_selection!(queue; family expected_queue_family_index, expected_queue_index);
        }

        #[test]
        fn success_all_same_family() {
            let window_id0 = WindowId::from(0);
            let window_id1 = WindowId::from(1);
            let window_id2 = WindowId::from(2);

            let mut queues = BTreeMap::<WindowId, QueueSelection>::new();

            let queue_family_properties = queue_family_properties! {
                8 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING, // all surfaces get assigned to this queue
                2 for QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER
            };

            let surface_support = btreemap! {
                (window_id0, 0) => true,
                (window_id1, 0) => true,
                (window_id2, 0) => true,
                (window_id0, 1) => false,
                (window_id1, 1) => false,
                (window_id2, 1) => false,
                (window_id0, 2) => true,
                (window_id1, 2) => true,
                (window_id2, 2) => true,
            };

            test_fn(&queue_family_properties, &window_id0, &surface_support, &mut queues, 0, 0);
            test_fn(&queue_family_properties, &window_id1, &surface_support, &mut queues, 0, 1);
            test_fn(&queue_family_properties, &window_id2, &surface_support, &mut queues, 0, 2);
        }

        #[test]
        fn success_different_families() {
            let window_id0 = WindowId::from(0);
            let window_id1 = WindowId::from(1);
            let window_id2 = WindowId::from(2);

            let mut queues = BTreeMap::<WindowId, QueueSelection>::new();

            let queue_family_properties = queue_family_properties! {
                2 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING, // only two surfaces get assigned to this queue
                2 for QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER // the last surface gets assigned to this queue
            };

            let surface_support = btreemap! {
                (window_id0, 0) => true,
                (window_id1, 0) => true,
                (window_id2, 0) => true,
                (window_id0, 1) => false,
                (window_id1, 1) => false,
                (window_id2, 1) => false,
                (window_id0, 2) => true,
                (window_id1, 2) => true,
                (window_id2, 2) => true,
            };

            test_fn(&queue_family_properties, &window_id0, &surface_support, &mut queues, 0, 0);
            test_fn(&queue_family_properties, &window_id1, &surface_support, &mut queues, 0, 1);
            test_fn(&queue_family_properties, &window_id2, &surface_support, &mut queues, 2, 0);
        }
    }

    mod find_least_stressed_presentation_queue {
        use super::*;

        #[test]
        fn success() {
            let window_id = WindowId::from(0);

            #[rustfmt::skip]
            let queues = btreemap! {
                WindowId::from(0) => QueueSelection { queue_family_index: 0, queue_index: 0 },
                WindowId::from(1) => QueueSelection { queue_family_index: 0, queue_index: 0 },
                WindowId::from(2) => QueueSelection { queue_family_index: 0, queue_index: 1 },
                WindowId::from(3) => QueueSelection { queue_family_index: 0, queue_index: 2 },
                WindowId::from(4) => QueueSelection { queue_family_index: 0, queue_index: 3 },
                WindowId::from(5) => QueueSelection { queue_family_index: 0, queue_index: 3 },
            };

            let queue_family_properties = queue_family_properties! {
                4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING
            };

            let surface_support = btreemap! {
                (window_id, 0) => true,
            };

            let queue = find_least_stressed_presentation_queue(&queue_family_properties, &window_id, &surface_support, &queues).unwrap();
            assert_eq!(queue.queue_family_index, 0);
            assert!(vec![1, 2].contains(&queue.queue_index));
        }
    }

    mod plan_queues {
        use super::*;

        #[test]
        fn all_on_single_queue_family() {
            let window_id0 = WindowId::from(0);
            let window_id1 = WindowId::from(1);
            let window_id2 = WindowId::from(2);

            let surfaces = vec![window_id0, window_id1, window_id2];

            let queue_family_properties = queue_family_properties! {
                8 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                2 for QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING,
                4 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER
            };

            let surface_support = btreemap! {
                (window_id0, 0) => true,
                (window_id1, 0) => true,
                (window_id2, 0) => true,
                (window_id0, 1) => false,
                (window_id1, 1) => false,
                (window_id2, 1) => false,
                (window_id0, 2) => true,
                (window_id1, 2) => true,
                (window_id2, 2) => true,
            };

            let queues_plan = plan_queues(surfaces.iter(), queue_family_properties, &surface_support).unwrap();
            assert!(queues_plan.presentation_queues.len() == 3);
            assert!(queues_plan.presentation_queue_mapping.len() == 3);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id0]]; family 0, 0);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id1]]; family 0, 1);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id2]]; family 0, 2);
            assert_queue_selection!(queues_plan.transfer_queue; family 1, 0);
        }

        #[test]
        fn all_on_single_queue() {
            let window_id0 = WindowId::from(0);
            let window_id1 = WindowId::from(1);
            let window_id2 = WindowId::from(2);

            let surfaces = vec![window_id0, window_id1, window_id2];

            let queue_family_properties = queue_family_properties! {
                1 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING
            };

            let surface_support = btreemap! {
                (window_id0, 0) => true,
                (window_id1, 0) => true,
                (window_id2, 0) => true,
            };

            let queues_plan = plan_queues(surfaces.iter(), queue_family_properties, &surface_support).unwrap();
            assert!(queues_plan.presentation_queues.len() == 1);
            assert!(queues_plan.presentation_queue_mapping.len() == 3);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id0]]; family 0, 0);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id1]]; family 0, 0);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id2]]; family 0, 0);
            assert_queue_selection!(queues_plan.transfer_queue; family 0, 0);
        }

        #[test]
        fn evenly_distributed() {
            let window_id0 = WindowId::from(0);
            let window_id1 = WindowId::from(1);
            let window_id2 = WindowId::from(2);
            let window_id3 = WindowId::from(3);

            let surfaces = vec![window_id0, window_id1, window_id2, window_id3];

            let queue_family_properties = queue_family_properties! {
                2 for QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER | QueueFlags::SPARSE_BINDING
            };

            let surface_support = btreemap! {
                (window_id0, 0) => true,
                (window_id1, 0) => true,
                (window_id2, 0) => true,
                (window_id3, 0) => true,
            };

            let queues_plan = plan_queues(surfaces.iter(), queue_family_properties, &surface_support).unwrap();
            assert!(queues_plan.presentation_queues.len() == 2);
            assert!(queues_plan.presentation_queue_mapping.len() == 4);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id0]]; family 0, 0);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id1]]; family 0, 1);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id2]]; family 0, 0);
            assert_queue_selection!(queues_plan.presentation_queues[queues_plan.presentation_queue_mapping[&window_id3]]; family 0, 1);
            assert_queue_selection!(queues_plan.transfer_queue; family 0, 0);
        }
    }
}
