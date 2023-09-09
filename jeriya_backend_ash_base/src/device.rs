use std::sync::Arc;

use ash::{
    extensions::khr,
    vk::{self, PhysicalDeviceFeatures2, PhysicalDeviceShaderDrawParametersFeatures},
};

use crate::{instance::Instance, physical_device::PhysicalDevice, AsRawVulkan, Error, Extensions, PhysicalDeviceFeature};

pub struct Device {
    device: ash::Device,
    pub physical_device: PhysicalDevice,
    pub extensions: Extensions,
    instance: Arc<Instance>,
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { self.device.destroy_device(None) };
    }
}

impl Device {
    /// Creates a new `Device` based on the given [`PhysicalDevice`].
    pub fn new(physical_device: PhysicalDevice, instance: &Arc<Instance>) -> crate::Result<Arc<Self>> {
        let features = {
            let available_features = unsafe {
                instance
                    .as_raw_vulkan()
                    .get_physical_device_features(*physical_device.as_raw_vulkan())
            };
            if available_features.wide_lines != vk::TRUE {
                return Err(Error::PhysicalDeviceFeatureMissing(PhysicalDeviceFeature::WideLines));
            }
            if available_features.shader_int64 != vk::TRUE {
                return Err(Error::PhysicalDeviceFeatureMissing(PhysicalDeviceFeature::ShaderInt64));
            }
            if available_features.multi_draw_indirect != vk::TRUE {
                return Err(Error::PhysicalDeviceFeatureMissing(PhysicalDeviceFeature::MultiDrawIndirect));
            }
            vk::PhysicalDeviceFeatures::builder()
                .wide_lines(true)
                .shader_int64(true)
                .multi_draw_indirect(true)
        };

        let features2 = {
            let available_features = unsafe {
                let mut shader_draw_parameters = PhysicalDeviceShaderDrawParametersFeatures::builder()
                    .shader_draw_parameters(true)
                    .build();

                let mut features = PhysicalDeviceFeatures2::builder().push_next(&mut shader_draw_parameters).build();
                instance
                    .as_raw_vulkan()
                    .get_physical_device_features2(*physical_device.as_raw_vulkan(), &mut features);

                // dbg!(shader_draw_parameters.shader_draw_parameters);
                // panic!();
            };
        };

        let queue_priorities = physical_device
            .suitable_presentation_graphics_queue_family_infos
            .iter()
            .map(|suitable_queue_family_info| {
                std::iter::repeat(1.0)
                    .take(suitable_queue_family_info.queue_count as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let queue_infos = physical_device
            .suitable_presentation_graphics_queue_family_infos
            .iter()
            .zip(queue_priorities.iter())
            .map(|(suitable_queue_family_info, priorities)| {
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(suitable_queue_family_info.queue_family_index)
                    .queue_priorities(priorities)
                    .build()
            })
            .collect::<Vec<_>>();
        let device_extension_names_raw = [khr::Swapchain::name().as_ptr(), khr::PushDescriptor::name().as_ptr()];

        let mut shader_draw_parameters = PhysicalDeviceShaderDrawParametersFeatures::builder()
            .shader_draw_parameters(true)
            .build();

        let device_create_info = vk::DeviceCreateInfo::builder()
            .push_next(&mut shader_draw_parameters)
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extension_names_raw)
            .enabled_features(&features);
        let device = unsafe {
            instance
                .as_raw_vulkan()
                .create_device(*physical_device.as_raw_vulkan(), &device_create_info, None)?
        };

        let extensions = Extensions::new(instance.as_raw_vulkan(), &device);

        Ok(Arc::new(Device {
            device,
            physical_device,
            instance: instance.clone(),
            extensions,
        }))
    }

    /// Wait for a device to become idle
    pub fn wait_for_idle(&self) -> crate::Result<()> {
        Ok(unsafe { self.device.device_wait_idle() }?)
    }

    /// Returns the [`Instance`] on which the `Device` was created.
    pub fn instance(&self) -> &Arc<Instance> {
        &self.instance
    }

    /// Find a memory type for the given memory requirements
    pub fn find_memorytype_index(
        &self,
        memory_requirements: &vk::MemoryRequirements,
        memory_properties: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        // Try to find an exactly matching memory flag
        let perfect_match = find_memorytype_index_by(
            &self.physical_device.physical_device_memory_properties,
            memory_requirements,
            memory_properties,
            |property_flags, flags| property_flags == flags,
        );
        if let Some(best_suitable_index) = perfect_match {
            Some(best_suitable_index)
        } else {
            // Otherwise find a memory flag that works
            find_memorytype_index_by(
                &self.physical_device.physical_device_memory_properties,
                memory_requirements,
                memory_properties,
                |property_flags, flags| property_flags & flags == flags,
            )
        }
    }
}

fn find_memorytype_index_by<F: Fn(vk::MemoryPropertyFlags, vk::MemoryPropertyFlags) -> bool>(
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    memory_req: &vk::MemoryRequirements,
    flags: vk::MemoryPropertyFlags,
    f: F,
) -> Option<u32> {
    let mut memory_type_bits = memory_req.memory_type_bits;
    for (index, memory_type) in memory_prop.memory_types.iter().enumerate() {
        if memory_type_bits & 1 == 1 && f(memory_type.property_flags, flags) {
            return Some(index as u32);
        }
        memory_type_bits >>= 1;
    }
    None
}

impl AsRawVulkan for Device {
    type Output = ash::Device;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.device
    }
}

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use jeriya_shared::winit::window::Window;
    use jeriya_test::create_window;

    use crate::{device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface};

    /// Test fixture for a [`Device`] and all its dependencies
    pub struct TestFixtureDevice {
        pub window: Window,
        pub entry: Arc<Entry>,
        pub instance: Arc<Instance>,
        pub surface: Arc<Surface>,
        pub device: Arc<Device>,
    }

    impl TestFixtureDevice {
        pub fn new() -> crate::Result<Self> {
            let window = create_window();
            let entry = Entry::new()?;
            let instance = Instance::new(&entry, "my_application", true)?;
            let surface = Surface::new(&entry, &instance, &window)?;
            let physical_device = PhysicalDevice::new(&instance, std::iter::once(&surface))?;
            let device = Device::new(physical_device, &instance)?;
            Ok(Self {
                window,
                entry,
                instance,
                surface,
                device,
            })
        }
    }

    mod new {
        use super::TestFixtureDevice;

        #[test]
        fn smoke() {
            let _device_test_fixture = TestFixtureDevice::new().unwrap();
        }
    }
}
