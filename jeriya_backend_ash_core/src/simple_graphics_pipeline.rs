use ash::vk::{self};
use jeriya_shared::{debug_info, AsDebugInfo, DebugInfo};

use std::{ffi::CString, io::Cursor, sync::Arc};

use crate::{
    device::Device, shader_module::ShaderModule, swapchain::Swapchain, swapchain_render_pass::SwapchainRenderPass, AsRawVulkan,
    DebugInfoAshExtension,
};

#[repr(C)]
#[derive(Debug, Default)]
pub struct PushConstants {
    _non_zero: u32,
}

pub struct SimpleGraphicsPipeline {
    _vertex_shader: ShaderModule,
    _fragment_shader: ShaderModule,
    graphics_pipeline: vk::Pipeline,
    graphics_pipeline_layout: vk::PipelineLayout,
    debug_info: DebugInfo,
    device: Arc<Device>,
}

impl Drop for SimpleGraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            let device = &self.device.as_raw_vulkan();
            device.destroy_pipeline(self.graphics_pipeline, None);
            device.destroy_pipeline_layout(self.graphics_pipeline_layout, None);
        }
    }
}

impl SimpleGraphicsPipeline {
    pub fn new(
        device: &Arc<Device>,
        renderpass: &SwapchainRenderPass,
        swapchain: &Swapchain,
        debug_info: DebugInfo,
    ) -> crate::Result<Self> {
        let entry_name = CString::new("main").expect("Valid c string");

        let vertex_shader_spirv = include_bytes!("../test_data/red_triangle.vert.spv").to_vec();
        let fragment_shader_spirv = include_bytes!("../test_data/red_triangle.frag.spv").to_vec();
        let vertex_shader = ShaderModule::new(
            device,
            Cursor::new(&vertex_shader_spirv),
            debug_info!("SimpleGraphicsPipeline-vertex-ShaderModule"),
        )?;
        let fragment_shader = ShaderModule::new(
            device,
            Cursor::new(&fragment_shader_spirv),
            debug_info!("SimpleGraphicsPipeline-fragment-ShaderModule"),
        )?;

        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                module: *vertex_shader.as_raw_vulkan(),
                p_name: entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                module: *fragment_shader.as_raw_vulkan(),
                p_name: entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];
        let descriptor_set_layouts = [];

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
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
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
            cull_mode: vk::CullModeFlags::BACK,
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

        let dynamic_state = [];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

        let vertex_input_binding_descriptions = [];
        let vertex_input_attribute_descriptions = [];

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
        })
    }
}

impl AsRawVulkan for SimpleGraphicsPipeline {
    type Output = vk::Pipeline;
    fn as_raw_vulkan(&self) -> &Self::Output {
        &self.graphics_pipeline
    }
}

impl AsDebugInfo for SimpleGraphicsPipeline {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use jeriya_shared::debug_info;

        use crate::{
            device::tests::TestFixtureDevice, simple_graphics_pipeline::SimpleGraphicsPipeline, swapchain::Swapchain,
            swapchain_render_pass::SwapchainRenderPass,
        };

        #[test]
        fn smoke() {
            let test_fixture_device = TestFixtureDevice::new().unwrap();
            let swapchain = Swapchain::new(&test_fixture_device.device, &test_fixture_device.surface, 2, None).unwrap();
            let render_pass = SwapchainRenderPass::new(&test_fixture_device.device, &swapchain).unwrap();
            let _graphics_pipeline = SimpleGraphicsPipeline::new(
                &test_fixture_device.device,
                &render_pass,
                &swapchain,
                debug_info!("my_graphics_pipeline"),
            )
            .unwrap();
        }
    }
}