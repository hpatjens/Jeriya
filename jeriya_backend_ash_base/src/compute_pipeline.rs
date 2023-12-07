use std::{ffi::CString, io::Cursor, sync::Arc};

use ash::vk;
use jeriya_shared::{debug_info, nalgebra::Vector4, AsDebugInfo, DebugInfo};

use crate::{
    descriptor_set_layout::DescriptorSetLayout,
    device::Device,
    shader_interface::{self, Camera, CameraInstance, MeshAttributes, PerFrameData, PointCloudPage, RigidMesh, RigidMeshInstance},
    shader_module::ShaderModule,
    specialization_constants::SpecializationConstants,
    AsRawVulkan,
};

pub trait ComputePipeline {
    fn compute_pipeline(&self) -> vk::Pipeline;
    fn pipeline_layout(&self) -> vk::PipelineLayout;
}

#[derive(Debug, Clone)]
pub struct GenericComputePipelineConfig {
    pub shader_spirv: Arc<Vec<u8>>,
    pub debug_info: DebugInfo,
}

pub struct GenericComputePipeline {
    config: GenericComputePipelineConfig,
    pipeline_layout: vk::PipelineLayout,
    compute_pipeline: vk::Pipeline,
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
    device: Arc<Device>,
}

impl GenericComputePipeline {
    pub fn new(
        device: &Arc<Device>,
        config: &GenericComputePipelineConfig,
        specialization_constants: &SpecializationConstants,
    ) -> crate::Result<Self> {
        let entry_name = CString::new("main").expect("Valid c string");

        let shader = ShaderModule::new(
            device,
            Cursor::new(&config.shader_spirv.as_ref()),
            debug_info!("GenericComputePipeline-ShaderModule"),
        )?;

        let specialization_info = vk::SpecializationInfo::builder()
            .map_entries(specialization_constants.map_entries())
            .data(specialization_constants.data())
            .build();

        let shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(*shader.as_raw_vulkan())
            .name(entry_name.as_c_str())
            .specialization_info(&specialization_info)
            .build();

        let descriptor_set_layout = Arc::new(
            DescriptorSetLayout::builder()
                .push_uniform_buffer::<PerFrameData>(0, 1)
                .push_storage_buffer::<Camera>(1, 1)
                .push_storage_buffer::<CameraInstance>(2, 1)
                .push_storage_buffer::<u32>(3, 1)
                .push_storage_buffer::<Vector4<f32>>(5, 1)
                .push_storage_buffer::<u32>(6, 1)
                .push_storage_buffer::<Vector4<f32>>(7, 1)
                .push_storage_buffer::<MeshAttributes>(8, 1)
                .push_storage_buffer::<RigidMesh>(9, 1)
                .push_storage_buffer::<u32>(10, 1)
                .push_storage_buffer::<RigidMeshInstance>(11, 1)
                .push_storage_buffer::<shader_interface::Meshlet>(12, 1)
                .push_storage_buffer::<u32>(13, 1)
                .push_storage_buffer::<u32>(14, 1)
                .push_storage_buffer::<u32>(15, 1)
                .push_storage_buffer::<shader_interface::PointCloud>(16, 1)
                .push_storage_buffer::<shader_interface::PointCloudInstance>(17, 1)
                .push_storage_buffer::<u32>(18, 1)
                .push_storage_buffer::<shader_interface::PointCloudAttributes>(19, 1)
                .push_storage_buffer::<Vector4<f32>>(20, 1)
                .push_storage_buffer::<Vector4<f32>>(21, 1)
                .push_storage_buffer::<PointCloudPage>(22, 1)
                .push_storage_buffer::<u32>(23, 1)
                .push_storage_buffer::<PointCloudPage>(24, 1)
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
            config: config.clone(),
            compute_pipeline,
            pipeline_layout,
            descriptor_set_layout,
            device: device.clone(),
        })
    }
}

impl Drop for GenericComputePipeline {
    fn drop(&mut self) {
        unsafe {
            let device = &self.device.as_raw_vulkan();
            device.destroy_pipeline(self.compute_pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

impl ComputePipeline for GenericComputePipeline {
    fn compute_pipeline(&self) -> vk::Pipeline {
        self.compute_pipeline
    }

    fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }
}

impl AsDebugInfo for GenericComputePipeline {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.config.debug_info
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::sync::Arc;

        use jeriya_shared::debug_info;

        use crate::{
            compute_pipeline::GenericComputePipeline, compute_pipeline::GenericComputePipelineConfig, device::TestFixtureDevice,
            specialization_constants::SpecializationConstants,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let config = GenericComputePipelineConfig {
                shader_spirv: Arc::new(include_bytes!("../test_data/cull_rigid_mesh_instances.comp.spv").to_vec()),
                debug_info: debug_info!("my_compute_pipeline"),
            };
            let specialization_constants = SpecializationConstants::new();
            let _compute_pipeline = GenericComputePipeline::new(&test_fixture_device.device, &config, &specialization_constants).unwrap();
        }
    }
}
