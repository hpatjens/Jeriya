use ash::vk;
use jeriya_shared::bitflags::bitflags;

bitflags! {
    /// Flags that specify the usage of a buffer
    pub struct BufferUsageFlags: u32 {
        // WARNING: Has to match the Vulkan flags by value
        // https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkBufferUsageFlagBits.html
        const VERTEX_BUFFER = 0x00000080;
    }
}

impl From<BufferUsageFlags> for vk::BufferUsageFlags {
    fn from(flags: BufferUsageFlags) -> Self {
        vk::BufferUsageFlags::from_raw(flags.bits())
    }
}
