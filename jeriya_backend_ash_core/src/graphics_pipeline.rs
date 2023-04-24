use ash::vk;

use crate::AsRawVulkan;

pub trait GraphicsPipeline: AsRawVulkan<Output = vk::Pipeline> {}
