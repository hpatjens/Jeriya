use ash::vk;

pub trait ComputePipeline {
    fn compute_pipeline(&self) -> vk::Pipeline;
    fn compute_pipeline_layout(&self) -> vk::PipelineLayout;
}
