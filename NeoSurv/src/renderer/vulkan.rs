pub(crate) const NAME: &str = "Vulkan";

pub(crate) fn backends() -> wgpu::Backends {
    wgpu::Backends::VULKAN
}
