use std::rc::Rc;
use std::{slice, ptr};

use {gl};
use core::{self as c, device as d, image as i, pass, pso, buffer, mapping};
use core::memory::{self, Bind, SHADER_RESOURCE, UNORDERED_ACCESS, Typed};
use core::format::{ChannelType, Format};
use core::target::{Layer, Level};

use {Info, Backend as B, Share};
use {conv, device, native as n, pool};

fn access_to_map_bits(access: memory::Access) -> gl::types::GLenum {
    let mut r = 0;
    if access.contains(memory::READ) { r |= gl::MAP_READ_BIT; }
    if access.contains(memory::WRITE) { r |= gl::MAP_WRITE_BIT; }
    r
}

fn access_to_gl(access: memory::Access) -> gl::types::GLenum {
    match access {
        memory::RW => gl::READ_WRITE,
        memory::READ => gl::READ_ONLY,
        memory::WRITE => gl::WRITE_ONLY,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct UnboundBuffer;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct UnboundImage;

/// GL device.
pub struct Device {
    share: Rc<Share>,
}

impl Clone for Device {
    fn clone(&self) -> Device {
        Device::new(self.share.clone())
    }
}

impl Device {
    /// Create a new `Device`.
    pub fn new(share: Rc<Share>) -> Device {
        Device {
            share: share,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MappingKind {}

#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct Mapping {
    pub kind: MappingKind,
    pub pointer: *mut ::std::os::raw::c_void,
}

unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}


impl d::Device<B> for Device {
    fn get_features(&self) -> &c::Features {
        &self.share.features
    }

    fn get_limits(&self) -> &c::Limits {
        &self.share.limits
    }

    fn create_heap(&mut self, _: &c::HeapType, _: d::ResourceHeapType, _: u64) -> Result<n::Heap, d::ResourceHeapError> {
        unimplemented!()
    }

    fn create_renderpass(&mut self, _: &[pass::Attachment], _: &[pass::SubpassDesc], _: &[pass::SubpassDependency]) -> n::RenderPass {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self, _: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(&mut self, _: &[(&n::ShaderLib, &n::PipelineLayout, pass::SubPass<'a, B>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
                unimplemented!()
            }

    fn create_compute_pipelines(&mut self, _: &[(&n::ShaderLib, pso::EntryPoint, &n::PipelineLayout)]) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &mut self,
        _: &n::RenderPass,
        _: &[&n::RenderTargetView],
        _: &[&n::DepthStencilView],
        _: u32, _: u32, _: u32
    ) -> n::FrameBuffer {
        unimplemented!()
    }

    fn create_sampler(&mut self, info: i::SamplerInfo) -> n::Sampler {
        unimplemented!()
    }
    fn create_buffer(&mut self, _: u64, _: u64, _: buffer::Usage) -> Result<device::UnboundBuffer, buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&mut self, _: &device::UnboundBuffer) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_buffer_memory(&mut self, _: &n::Heap, _: u64, _: device::UnboundBuffer) -> Result<n::Buffer, buffer::CreationError> {
        unimplemented!()
    }

    fn create_image(&mut self, _: i::Kind, _: i::Level, _: Format, _: i::Usage)
         -> Result<device::UnboundImage, i::CreationError> {
            unimplemented!()
         }

    fn get_image_requirements(&mut self, _: &device::UnboundImage) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_image_memory(&mut self, _: &n::Heap, _: u64, _: device::UnboundImage) -> Result<n::Image, i::CreationError> {
        unimplemented!()
    }

    fn view_buffer_as_constant(&mut self, _: &n::Buffer, _: usize, _: usize) -> Result<n::ConstantBufferView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, _: &n::Image, _: Format, _: i::SubresourceRange) -> Result<n::RenderTargetView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_shader_resource(&mut self, _: &n::Image, _: Format) -> Result<n::ShaderResourceView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_unordered_access(&mut self, _: &n::Image, _: Format) -> Result<n::UnorderedAccessView, d::TargetViewError> {
        unimplemented!()
    }
    fn create_descriptor_pool(&mut self, _: usize, _: &[pso::DescriptorRangeDesc]) -> n::DescriptorPool {
        unimplemented!()
    }
    fn create_descriptor_set_layout(&mut self, _: &[pso::DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
        unimplemented!()
    }


    fn update_descriptor_sets(&mut self, _: &[pso::DescriptorSetWrite<B>]) {
        unimplemented!()
    }

    fn read_mapping<'a, T>(&self, _: &'a n::Buffer, _: u64, _: u64)
                           -> Result<mapping::Reader<'a, B, T>, mapping::Error>
        where T: Copy {
            unimplemented!()
        }

    fn write_mapping<'a, 'b, T>(&mut self, _: &'a n::Buffer, _: u64, _: u64)
                                -> Result<mapping::Writer<'a, B, T>, mapping::Error>
        where T: Copy {
            unimplemented!()
        }

    fn create_semaphore(&mut self) -> n::Semaphore {
        unimplemented!()
    }

    fn create_fence(&mut self, _: bool) -> n::Fence {
        unimplemented!()
    }

    fn reset_fences(&mut self, _: &[&n::Fence]) {
        unimplemented!()
    }
    fn wait_for_fences(&mut self, _: &[&n::Fence], _: d::WaitFor, _: u32) -> bool {
        unimplemented!()
    }

    fn destroy_heap(&mut self, _: n::Heap) {
        unimplemented!()
    }

    fn destroy_shader_lib(&mut self, _: n::ShaderLib) {
        unimplemented!()
    }

    fn destroy_renderpass(&mut self, _: n::RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&mut self, _: n::PipelineLayout) {
        unimplemented!()
    }
    fn destroy_graphics_pipeline(&mut self, _: n::GraphicsPipeline) {
        unimplemented!()
    }
    fn destroy_compute_pipeline(&mut self, _: n::ComputePipeline) {
        unimplemented!()
    }
    fn destroy_framebuffer(&mut self, _: n::FrameBuffer) {
        unimplemented!()
    }
    fn destroy_buffer(&mut self, _: n::Buffer) {
        unimplemented!()
    }
    fn destroy_image(&mut self, _: n::Image) {
        unimplemented!()
    }

    fn destroy_render_target_view(&mut self, _: n::RenderTargetView) {
        unimplemented!()
    }

    fn destroy_depth_stencil_view(&mut self, _: n::DepthStencilView) {
        unimplemented!()
    }

    fn destroy_constant_buffer_view(&mut self, _: n::ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, _: n::ShaderResourceView) {
        unimplemented!()
    }

    fn destroy_unordered_access_view(&mut self, _: n::UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, _: n::Sampler) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&mut self, _: n::DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&mut self, _: n::DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&mut self, _: n::Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&mut self, _: n::Semaphore) {
        unimplemented!()
    }
}

pub fn wait_fence(fence: &n::Fence, gl: &gl::Gl, timeout_ms: u32) -> gl::types::GLenum {
    let timeout = timeout_ms as u64 * 1_000_000;
    // TODO:
    // This can be called by multiple objects wanting to ensure they have exclusive
    // access to a resource. How much does this call costs ? The status of the fence
    // could be cached to avoid calling this more than once (in core or in the backend ?).
    unsafe { gl.ClientWaitSync(fence.0.get(), gl::SYNC_FLUSH_COMMANDS_BIT, timeout) }
}
