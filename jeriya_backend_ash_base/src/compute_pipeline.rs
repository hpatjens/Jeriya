use std::{ffi::CString, io::Cursor, sync::Arc};

use ash::vk;
use jeriya_shared::{debug_info, nalgebra::Vector4, AsDebugInfo, DebugInfo, RendererConfig};

use crate::{
    descriptor_set_layout::DescriptorSetLayout,
    device::Device,
    shader_interface::{self, Camera, CameraInstance, MeshAttributes, PerFrameData, RigidMesh, RigidMeshInstance},
    shader_module::ShaderModule,
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
    pub fn new(device: &Arc<Device>, config: &GenericComputePipelineConfig, _renderer_config: &RendererConfig) -> crate::Result<Self> {
        let entry_name = CString::new("main").expect("Valid c string");

        let shader = ShaderModule::new(
            device,
            Cursor::new(&config.shader_spirv.as_ref()),
            debug_info!("GenericComputePipeline-ShaderModule"),
        )?;

        // let specialization_constants = [
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(0)
        //         .offset(0)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(1)
        //         .offset(1 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(3)
        //         .offset(2 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(4)
        //         .offset(3 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(5)
        //         .offset(4 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(6)
        //         .offset(5 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(7)
        //         .offset(6 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        //     vk::SpecializationMapEntry::builder()
        //         .constant_id(8)
        //         .offset(7 * mem::size_of::<u32>() as u32)
        //         .size(std::mem::size_of::<u32>())
        //         .build(),
        // ];
        // let mut specialization_data = Vec::<u8>::new();
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_number_of_cameras as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_number_of_camera_instances as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_number_of_rigid_meshes as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_number_of_mesh_attributes as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_number_of_rigid_mesh_instances as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_meshlets as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_visible_rigid_mesh_instances as u32)
        //     .expect("failed to write specialization constant");
        // specialization_data
        //     .write_u32::<LittleEndian>(renderer_config.maximum_visible_rigid_mesh_meshlets as u32)
        //     .expect("failed to write specialization constant");
        // let specialization_info = vk::SpecializationInfo::builder()
        //     .map_entries(&specialization_constants)
        //     .data(&specialization_data)
        //     .build();

        let shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(*shader.as_raw_vulkan())
            .name(entry_name.as_c_str())
            // .specialization_info(&specialization_info)
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

        use jeriya_shared::{debug_info, RendererConfig};

        use crate::{compute_pipeline::GenericComputePipeline, compute_pipeline::GenericComputePipelineConfig, device::TestFixtureDevice};

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let config = GenericComputePipelineConfig {
                shader_spirv: Arc::new(include_bytes!("../test_data/cull_rigid_mesh_instances.comp.spv").to_vec()),
                debug_info: debug_info!("my_compute_pipeline"),
            };
            let _compute_pipeline = GenericComputePipeline::new(&test_fixture_device.device, &config, &RendererConfig::default()).unwrap();
        }
    }
}
