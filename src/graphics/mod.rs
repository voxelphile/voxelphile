#[cfg(feature = "vk")]
mod vulkan;
#[cfg(feature = "vk")]
pub type Graphics = vulkan::Vulkan;

pub trait GraphicsInterface {
    fn init() -> Self;
}