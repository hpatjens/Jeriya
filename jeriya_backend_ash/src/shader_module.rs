use ash::{util::*, vk};
use jeriya_shared::{AsDebugInfo, DebugInfo};

use std::{io, sync::Arc};

use crate::{device::Device, AsRawVulkan, DebugInfoAshExtension};

pub struct ShaderModule {
    shader_module: vk::ShaderModule,
    debug_info: DebugInfo,
    device: Arc<Device>,
}

impl ShaderModule {
    pub fn new<R>(device: &Arc<Device>, mut byte_code: R, debug_info: DebugInfo) -> crate::Result<Self>
    where
        R: io::Read + io::Seek,
    {
        let code = read_spv(&mut byte_code).map_err(|_| crate::Error::SpirvDecode)?;
        let shader_info = vk::ShaderModuleCreateInfo::builder().code(&code);
        let shader_module = unsafe { device.as_raw_vulkan().create_shader_module(&shader_info, None)? };
        let debug_info = debug_info.with_vulkan_ptr(shader_module);
        Ok(Self {
            shader_module,
            debug_info,
            device: device.clone(),
        })
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        unsafe {
            self.device.as_raw_vulkan().destroy_shader_module(self.shader_module, None);
        }
    }
}

impl AsRawVulkan for ShaderModule {
    type Output = vk::ShaderModule;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.shader_module
    }
}

impl AsDebugInfo for ShaderModule {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::io::Cursor;

        use jeriya_shared::debug_info;

        use crate::{device::TestFixtureDevice, shader_module::ShaderModule};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let spirv = include_bytes!("../test_data/red_triangle.frag.spv").to_vec();
            let _shader_module =
                ShaderModule::new(&test_fixture_device.device, Cursor::new(&spirv), debug_info!("my_shader_module")).unwrap();
        }
    }
}
