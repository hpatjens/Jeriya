use ash::vk;
use jeriya_content::common::AssetKey;
use jeriya_macros::profile;
use jeriya_shared::{
    debug_info,
    log::info,
    nalgebra::{Matrix4, Vector3, Vector4},
    AsDebugInfo, DebugInfo,
};

use std::{ffi::CString, io::Cursor, mem, sync::Arc};

use crate::{
    descriptor_set_layout::DescriptorSetLayout,
    device::Device,
    shader_interface::{self, Camera, CameraInstance, MeshAttributes, PerFrameData, PointCloudPage, RigidMesh, RigidMeshInstance},
    shader_module::ShaderModule,
    specialization_constants::SpecializationConstants,
    swapchain_render_pass::SwapchainRenderPass,
    AsRawVulkan,
};

#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct PushConstants {
    pub color: Vector4<f32>,
    pub matrix: Matrix4<f32>,
}

pub trait GraphicsPipeline {
    fn graphics_pipeline(&self) -> vk::Pipeline;
    fn pipeline_layout(&self) -> vk::PipelineLayout;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolygonMode {
    #[default]
    Fill,
    Line,
}

impl From<PolygonMode> for vk::PolygonMode {
    fn from(polygon_mode: PolygonMode) -> Self {
        match polygon_mode {
            PolygonMode::Fill => vk::PolygonMode::FILL,
            PolygonMode::Line => vk::PolygonMode::LINE,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    #[default]
    Back,
    FrontAndBack,
}

impl From<CullMode> for vk::CullModeFlags {
    fn from(cull_mode: CullMode) -> Self {
        match cull_mode {
            CullMode::None => vk::CullModeFlags::NONE,
            CullMode::Front => vk::CullModeFlags::FRONT,
            CullMode::Back => vk::CullModeFlags::BACK,
            CullMode::FrontAndBack => vk::CullModeFlags::FRONT_AND_BACK,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveTopology {
    #[default]
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

impl From<PrimitiveTopology> for vk::PrimitiveTopology {
    fn from(primitive_topology: PrimitiveTopology) -> Self {
        match primitive_topology {
            PrimitiveTopology::PointList => vk::PrimitiveTopology::POINT_LIST,
            PrimitiveTopology::LineList => vk::PrimitiveTopology::LINE_LIST,
            PrimitiveTopology::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
            PrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
            PrimitiveTopology::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
        }
    }
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct GenericGraphicsPipelineConfig {
    pub vertex_shader: Option<AssetKey>,
    pub fragment_shader: Option<AssetKey>,
    pub primitive_topology: PrimitiveTopology,
    pub polygon_mode: PolygonMode,
    pub cull_mode: CullMode,
    pub use_input_attributes: bool,
    pub use_dynamic_state_line_width: bool,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
}

pub struct GenericGraphicsPipeline {
    pub config: GenericGraphicsPipelineConfig,
    _vertex_shader: ShaderModule,
    _fragment_shader: ShaderModule,
    graphics_pipeline: vk::Pipeline,
    graphics_pipeline_layout: vk::PipelineLayout,
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
    device: Arc<Device>,
    debug_info: DebugInfo,
}

impl Drop for GenericGraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            let device = &self.device.as_raw_vulkan();
            device.destroy_pipeline(self.graphics_pipeline, None);
            device.destroy_pipeline_layout(self.graphics_pipeline_layout, None);
        }
    }
}

#[profile]
impl GenericGraphicsPipeline {
    pub fn new(
        device: &Arc<Device>,
        config: &GenericGraphicsPipelineConfig,
        vertex_shader_spirv: &[u8],
        fragment_shader_spirv: &[u8],
        renderpass: &SwapchainRenderPass,
        specialization_constants: &SpecializationConstants,
        debug_info: DebugInfo,
    ) -> crate::Result<Self> {
        let entry_name = CString::new("main").expect("Valid c string");

        info!("Create shader modules for GenericGraphicsPipeline \"{}\"", debug_info.name());
        let vertex_shader = ShaderModule::new(
            device,
            Cursor::new(vertex_shader_spirv),
            debug_info!("GenericGraphicsPipeline-vertex-ShaderModule"),
        )?;
        let fragment_shader = ShaderModule::new(
            device,
            Cursor::new(fragment_shader_spirv),
            debug_info!("GenericGraphicsPipeline-fragment-ShaderModule"),
        )?;

        let specialization_info = vk::SpecializationInfo::builder()
            .map_entries(specialization_constants.map_entries())
            .data(specialization_constants.data())
            .build();

        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                module: *vertex_shader.as_raw_vulkan(),
                p_name: entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                p_specialization_info: &specialization_info as *const _,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                module: *fragment_shader.as_raw_vulkan(),
                p_name: entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                p_specialization_info: &specialization_info as *const _,
                ..Default::default()
            },
        ];

        info!("Create pipeline layout for GenericGraphicsPipeline \"{}\"", debug_info.name());
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
                .push_storage_buffer::<u32>(25, 1)
                .push_storage_buffer::<u32>(26, 1)
                .push_storage_buffer::<shader_interface::FrameTelemetry>(27, 1)
                .push_storage_buffer::<f32>(28, 1)
                .build(device)?,
        );
        let descriptor_set_layouts = [*descriptor_set_layout.as_raw_vulkan()];

        let push_constant_range = [vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::ALL)
            .size(std::mem::size_of::<PushConstants>() as u32)
            .offset(0)
            .build()];

        let layout_create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: descriptor_set_layouts.len() as u32,
            p_set_layouts: descriptor_set_layouts.as_ptr(),
            push_constant_range_count: push_constant_range.len() as u32,
            p_push_constant_ranges: push_constant_range.as_ptr(),
            ..Default::default()
        };
        let graphics_pipeline_layout = unsafe { device.as_raw_vulkan().create_pipeline_layout(&layout_create_info, None)? };

        let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
            topology: config.primitive_topology.into(),
            ..Default::default()
        };

        // flipped viewport with VK_KHR_MAINTENANCE1 extension so that y is pointing up
        let viewports = vec![vk::Viewport {
            x: 0.0,
            y: config.framebuffer_height as f32,
            width: config.framebuffer_width as f32,
            height: -(config.framebuffer_height as f32),
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = vec![vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: config.framebuffer_width,
                height: config.framebuffer_height,
            },
        }];
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .scissors(&scissors)
            .viewports(&viewports);

        let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            polygon_mode: config.polygon_mode.into(),
            cull_mode: config.cull_mode.into(),
            ..Default::default()
        };
        let multisample_state_info = vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            ..Default::default()
        };
        let noop_stencil_state = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            ..Default::default()
        };
        let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: 1,
            depth_write_enable: 1,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            front: noop_stencil_state,
            back: noop_stencil_state,
            max_depth_bounds: 1.0,
            ..Default::default()
        };
        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            blend_enable: 0,
            src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ZERO,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        }];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states);

        let mut dynamic_state = Vec::new();
        if config.use_dynamic_state_line_width {
            dynamic_state.push(vk::DynamicState::LINE_WIDTH);
        }
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

        let mut vertex_input_binding_descriptions = Vec::new();
        if config.use_input_attributes {
            vertex_input_binding_descriptions.push(vk::VertexInputBindingDescription {
                binding: 0,
                stride: mem::size_of::<Vector3<f32>>() as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            });
        }

        let mut vertex_input_attribute_descriptions = Vec::new();
        if config.use_input_attributes {
            vertex_input_attribute_descriptions.push(vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            });
        }

        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_input_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_input_binding_descriptions)
            .build();

        let graphic_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stage_create_infos)
            .vertex_input_state(&vertex_input_state_info)
            .input_assembly_state(&vertex_input_assembly_state_info)
            .viewport_state(&viewport_state_info)
            .rasterization_state(&rasterization_info)
            .multisample_state(&multisample_state_info)
            .depth_stencil_state(&depth_state_info)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_info)
            .layout(graphics_pipeline_layout)
            .render_pass(*renderpass.as_raw_vulkan());

        info!("Create pipeline for GenericGraphicsPipeline \"{}\"", debug_info.name());
        let graphics_pipeline = unsafe {
            device
                .as_raw_vulkan()
                .create_graphics_pipelines(vk::PipelineCache::null(), &[graphic_pipeline_info.build()], None)
                .map_err(|(_, err)| err)?[0]
        };

        info!("Done creating GenericGraphicsPipeline \"{}\"", debug_info.name());
        Ok(Self {
            config: config.clone(),
            _vertex_shader: vertex_shader,
            _fragment_shader: fragment_shader,
            graphics_pipeline,
            graphics_pipeline_layout,
            descriptor_set_layout,
            device: device.clone(),
            debug_info,
        })
    }
}

impl GraphicsPipeline for GenericGraphicsPipeline {
    fn graphics_pipeline(&self) -> vk::Pipeline {
        self.graphics_pipeline
    }

    fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.graphics_pipeline_layout
    }
}

impl AsRawVulkan for GenericGraphicsPipeline {
    type Output = vk::Pipeline;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.graphics_pipeline
    }
}

impl AsDebugInfo for GenericGraphicsPipeline {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_content::common::AssetKey;
        use jeriya_shared::debug_info;

        use crate::{
            device::TestFixtureDevice,
            graphics_pipeline::{GenericGraphicsPipeline, GenericGraphicsPipelineConfig, PrimitiveTopology},
            specialization_constants::SpecializationConstants,
            swapchain::Swapchain,
            swapchain_render_pass::SwapchainRenderPass,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let swapchain = Swapchain::new(&test_fixture_device.device, &test_fixture_device.surface, 2, None).unwrap();
            let render_pass = SwapchainRenderPass::new(&test_fixture_device.device, &swapchain).unwrap();
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("vertex_shader")),
                fragment_shader: Some(AssetKey::new("fragment_shader")),
                primitive_topology: PrimitiveTopology::LineList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let specialization_constants = SpecializationConstants::new();
            let _graphics_pipeline = GenericGraphicsPipeline::new(
                &test_fixture_device.device,
                &config,
                include_bytes!("../test_data/red_triangle.vert.spv"),
                include_bytes!("../test_data/red_triangle.frag.spv"),
                &render_pass,
                &specialization_constants,
                debug_info!("my_graphics_pipeline"),
            )
            .unwrap();
        }
    }
}
