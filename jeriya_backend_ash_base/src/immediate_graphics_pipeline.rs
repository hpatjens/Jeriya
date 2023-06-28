use ash::vk::{self};
use jeriya_shared::{
    byteorder::{LittleEndian, WriteBytesExt},
    debug_info,
    nalgebra::{Matrix4, Vector3, Vector4},
    AsDebugInfo, DebugInfo, RendererConfig,
};

use std::{ffi::CString, io::Cursor, mem, sync::Arc};

use crate::{
    descriptor_set_layout::DescriptorSetLayout,
    device::Device,
    graphics_pipeline::GraphicsPipeline,
    shader_interface::{Camera, PerFrameData},
    shader_module::ShaderModule,
    swapchain::Swapchain,
    swapchain_render_pass::SwapchainRenderPass,
    AsRawVulkan, DebugInfoAshExtension,
};

pub enum Topology {
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct PushConstants {
    pub color: Vector4<f32>,
    pub matrix: Matrix4<f32>,
}

pub struct ImmediateGraphicsPipeline {
    _vertex_shader: ShaderModule,
    _fragment_shader: ShaderModule,
    graphics_pipeline: vk::Pipeline,
    pub(crate) graphics_pipeline_layout: vk::PipelineLayout,
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
    debug_info: DebugInfo,
    device: Arc<Device>,
}

impl Drop for ImmediateGraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            let device = &self.device.as_raw_vulkan();
            device.destroy_pipeline(self.graphics_pipeline, None);
            device.destroy_pipeline_layout(self.graphics_pipeline_layout, None);
        }
    }
}

impl ImmediateGraphicsPipeline {
    pub fn new(
        device: &Arc<Device>,
        renderpass: &SwapchainRenderPass,
        swapchain: &Swapchain,
        topology: Topology,
        renderer_config: &RendererConfig,
        debug_info: DebugInfo,
    ) -> crate::Result<Self> {
        let entry_name = CString::new("main").expect("Valid c string");

        let vertex_shader_spirv = include_bytes!("../test_data/color.vert.spv").to_vec();
        let fragment_shader_spirv = include_bytes!("../test_data/color.frag.spv").to_vec();
        let vertex_shader = ShaderModule::new(
            device,
            Cursor::new(&vertex_shader_spirv),
            debug_info!("ImmediateGraphicsPipeline-vertex-ShaderModule"),
        )?;
        let fragment_shader = ShaderModule::new(
            device,
            Cursor::new(&fragment_shader_spirv),
            debug_info!("ImmediateGraphicsPipeline-fragment-ShaderModule"),
        )?;
        let specialization_constants = [vk::SpecializationMapEntry::builder()
            .constant_id(0)
            .offset(0)
            .size(std::mem::size_of::<u32>())
            .build()];
        let mut specialization_data = Vec::<u8>::new();
        specialization_data
            .write_u32::<LittleEndian>(renderer_config.maximum_number_of_cameras as u32)
            .expect("failed to write specialization constant");
        let specialization_info = vk::SpecializationInfo::builder()
            .map_entries(&specialization_constants)
            .data(&specialization_data)
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
        let descriptor_set_layout = Arc::new(
            DescriptorSetLayout::builder()
                .push_uniform_buffer::<PerFrameData>(0, 1)
                .push_storage_buffer::<Camera>(1, 1)
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
            topology: match topology {
                Topology::LineList => vk::PrimitiveTopology::LINE_LIST,
                Topology::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
                Topology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
                Topology::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
            },
            ..Default::default()
        };

        let viewports = vec![vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: swapchain.extent().width as f32,
            height: swapchain.extent().height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = vec![vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: swapchain.extent().width,
                height: swapchain.extent().height,
            },
        }];

        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .scissors(&scissors)
            .viewports(&viewports);

        let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
            front_face: vk::FrontFace::CLOCKWISE,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
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

        let dynamic_state = [vk::DynamicState::LINE_WIDTH];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Vector3<f32>>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vertex_input_attribute_descriptions = [vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        }];

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

        let graphics_pipeline = unsafe {
            device
                .as_raw_vulkan()
                .create_graphics_pipelines(vk::PipelineCache::null(), &[graphic_pipeline_info.build()], None)
                .map_err(|(_, err)| err)?[0]
        };

        let debug_info = debug_info.with_vulkan_ptr(graphics_pipeline);
        Ok(Self {
            _vertex_shader: vertex_shader,
            _fragment_shader: fragment_shader,
            graphics_pipeline,
            graphics_pipeline_layout,
            debug_info,
            device: device.clone(),
            descriptor_set_layout,
        })
    }
}

impl GraphicsPipeline for ImmediateGraphicsPipeline {
    fn graphics_pipeline(&self) -> vk::Pipeline {
        self.graphics_pipeline
    }

    fn graphics_pipeline_layout(&self) -> vk::PipelineLayout {
        self.graphics_pipeline_layout
    }
}

impl AsRawVulkan for ImmediateGraphicsPipeline {
    type Output = vk::Pipeline;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.graphics_pipeline
    }
}

impl AsDebugInfo for ImmediateGraphicsPipeline {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_shared::{debug_info, RendererConfig};

        use crate::{
            device::tests::TestFixtureDevice,
            immediate_graphics_pipeline::{ImmediateGraphicsPipeline, Topology},
            swapchain::Swapchain,
            swapchain_render_pass::SwapchainRenderPass,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let swapchain = Swapchain::new(&test_fixture_device.device, &test_fixture_device.surface, 2, None).unwrap();
            let render_pass = SwapchainRenderPass::new(&test_fixture_device.device, &swapchain).unwrap();
            let _graphics_pipeline = ImmediateGraphicsPipeline::new(
                &test_fixture_device.device,
                &render_pass,
                &swapchain,
                Topology::LineList,
                &RendererConfig::default(),
                debug_info!("my_graphics_pipeline"),
            )
            .unwrap();
        }
    }
}
