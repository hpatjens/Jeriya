use ash::vk;

pub trait GraphicsPipeline {
    fn graphics_pipeline(&self) -> vk::Pipeline;
    fn graphics_pipeline_layout(&self) -> vk::PipelineLayout;
}
