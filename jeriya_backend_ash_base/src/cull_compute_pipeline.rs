use std::{ffi::CString, io::Cursor, sync::Arc};

use ash::vk;
use jeriya_shared::{debug_info, nalgebra::Vector4, AsDebugInfo, DebugInfo};

use crate::{
    compute_pipeline::ComputePipeline,
    descriptor_set_layout::DescriptorSetLayout,
    device::Device,
    shader_interface::{Camera, InanimateMesh, InanimateMeshInstance, PerFrameData},
    shader_module::ShaderModule,
    AsRawVulkan,
};

pub struct CullComputePipeline {
    pipeline_layout: vk::PipelineLayout,
    compute_pipeline: vk::Pipeline,
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
    debug_info: DebugInfo,
    device: Arc<Device>,
}

impl CullComputePipeline {
    pub fn new(device: &Arc<Device>, debug_info: DebugInfo) -> crate::Result<Self> {
        let entry_name = CString::new("main").expect("Valid c string");

        let shader_spirv = include_bytes!("../test_data/cull.comp.spv").to_vec();
        let shader = ShaderModule::new(device, Cursor::new(&shader_spirv), debug_info!("CullCompute-ShaderModule"))?;

        let shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(*shader.as_raw_vulkan())
            .name(entry_name.as_c_str())
            .build();

        let descriptor_set_layout = Arc::new(
            DescriptorSetLayout::builder()
                .push_uniform_buffer::<PerFrameData>(0, 1)
                .push_storage_buffer::<Camera>(1, 1)
                .push_storage_buffer::<InanimateMeshInstance>(2, 1)
                .push_storage_buffer::<crate::DrawIndirectCommand>(3, 1)
                .push_storage_buffer::<InanimateMesh>(4, 1)
                .push_storage_buffer::<Vector4<f32>>(5, 1)
                .build(device)?,
        );
        let descriptor_set_layouts = [*descriptor_set_layout.as_raw_vulkan()];

        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&descriptor_set_layouts).build();
        let pipeline_layout = unsafe { device.as_raw_vulkan().create_pipeline_layout(&pipeline_layout_create_info, None)? };

        let compute_pipeline_info = vk::ComputePipelineCreateInfo::builder()
            .stage(shader_stage_create_info)
            .layout(pipeline_layout)
            .build();
        let compute_pipeline = unsafe {
            device
                .as_raw_vulkan()
                .create_compute_pipelines(vk::PipelineCache::null(), &[compute_pipeline_info], None)
                .map_err(|(_, err)| err)?[0]
        };

        Ok(Self {
            compute_pipeline,
            pipeline_layout,
            descriptor_set_layout,
            debug_info,
            device: device.clone(),
        })
    }
}

impl Drop for CullComputePipeline {
    fn drop(&mut self) {
        unsafe {
            let device = &self.device.as_raw_vulkan();
            device.destroy_pipeline(self.compute_pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

impl ComputePipeline for CullComputePipeline {
    fn compute_pipeline(&self) -> vk::Pipeline {
        self.compute_pipeline
    }

    fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }
}

impl AsDebugInfo for CullComputePipeline {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_shared::debug_info;

        use crate::{cull_compute_pipeline::CullComputePipeline, device::tests::TestFixtureDevice};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let _compute_pipeline = CullComputePipeline::new(&test_fixture_device.device, debug_info!("my_compute_pipeline")).unwrap();
        }
    }
}
