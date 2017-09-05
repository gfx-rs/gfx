use std::rc::Rc;
use std::{slice, ptr};

use gl;
use gl::types::{GLenum, GLuint, GLint, GLfloat, GLsizei, GLvoid};
use core::{self as c, device as d, image as i, pass, pso, buffer, mapping};
use core::memory::{self, Bind, SHADER_RESOURCE, UNORDERED_ACCESS, Typed};
use core::format::{ChannelType, Format};
use core::target::{Layer, Level};

use {Info, Backend as B, Share};
use {conv, device, native as n, pool, state};

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
    pub(crate) fn new(share: Rc<Share>) -> Device {
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

    fn create_heap(&mut self, heap_type: &c::HeapType, resource_type: d::ResourceHeapType, size: u64) -> Result<n::Heap, d::ResourceHeapError> {
        Ok(n::Heap)
    }

    fn create_renderpass(&mut self, _: &[pass::Attachment], _: &[pass::SubpassDesc], _: &[pass::SubpassDependency]) -> n::RenderPass {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self, _: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(
        &mut self,
        descs: &[(&n::ShaderLib, &n::PipelineLayout, pass::SubPass<'a, B>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
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

    fn create_sampler(&mut self, info: i::SamplerInfo) -> n::FatSampler {
        if !self.share.features.sampler_objects {
            return n::FatSampler::Info(info);
        }

        let gl = &self.share.context;
        let mut name = 0 as n::Sampler;

        let (min, mag) = conv::filter_to_gl(info.filter);

        unsafe {
            gl.GenSamplers(1, &mut name);

            match info.filter{
                i::FilterMethod::Anisotropic(fac) if fac > 1 => {
                    if self.share.private_caps.sampler_anisotropy_ext {
                        gl.SamplerParameterf(name, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat);
                    } else if self.share.features.sampler_anisotropy {
                        // TODO: Uncomment once `gfx_gl` supports GL 4.6
                        // gl.SamplerParameterf(name, gl::TEXTURE_MAX_ANISOTROPY, fac as GLfloat);
                    }
                }
                _ => ()
            }

            gl.SamplerParameteri(name, gl::TEXTURE_MIN_FILTER, min as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_MAG_FILTER, mag as GLint);

            let (s, t, r) = info.wrap_mode;
            gl.SamplerParameteri(name, gl::TEXTURE_WRAP_S, conv::wrap_to_gl(s) as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_WRAP_T, conv::wrap_to_gl(t) as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_WRAP_R, conv::wrap_to_gl(r) as GLint);

            if self.share.features.sampler_lod_bias {
                gl.SamplerParameterf(name, gl::TEXTURE_LOD_BIAS, info.lod_bias.into());
            }
            if self.share.features.sampler_border_color {
                let border: [f32; 4] = info.border.into();
                gl.SamplerParameterfv(name, gl::TEXTURE_BORDER_COLOR, &border[0]);
            }

            let (min, max) = info.lod_range;
            gl.SamplerParameterf(name, gl::TEXTURE_MIN_LOD, min.into());
            gl.SamplerParameterf(name, gl::TEXTURE_MAX_LOD, max.into());

            match info.comparison {
                None => gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
                Some(cmp) => {
                    gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
                    gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
                }
            }
        }

        if let Err(err) = self.share.check() {
            panic!("Error {:?} creating sampler: {:?}", err, info)
        }

        n::FatSampler::Sampler(name)
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
        n::Semaphore
    }

    fn create_fence(&mut self, signalled: bool) -> n::Fence {
        let sync = if signalled && self.share.private_caps.sync_supported {
            let gl = &self.share.context;
            unsafe { gl.FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0) }
        } else {
            ptr::null()
        };
        n::Fence::new(sync)
    }

    fn reset_fences(&mut self, fences: &[&n::Fence]) {
        if !self.share.private_caps.sync_supported {
            return
        }

        let gl = &self.share.context;
        for fence in fences {
            let sync = fence.0.get();
            unsafe {
                if gl.IsSync(sync) == gl::TRUE {
                    gl.DeleteSync(sync);
                }
            }
            fence.0.set(ptr::null())
        }
    }

    fn wait_for_fences(&mut self, fences: &[&n::Fence], wait: d::WaitFor, timeout_ms: u32) -> bool {
        if !self.share.private_caps.sync_supported {
            return true;
        }

        match wait {
            d::WaitFor::All => {
                for fence in fences {
                    match wait_fence(fence, &self.share.context, timeout_ms) {
                        gl::TIMEOUT_EXPIRED => return false,
                        gl::WAIT_FAILED => {
                            if let Err(err) = self.share.check() {
                                error!("Error when waiting on fence: {:?}", err);
                            }
                            return false
                        }
                        _ => (),
                    }
                }
                // All fences have indicated a positive result
                true
            },
            d::WaitFor::Any => {
                let mut waiting = |timeout_ms: u32| {
                    for fence in fences {
                        match wait_fence(fence, &self.share.context, 0) {
                            gl::ALREADY_SIGNALED | gl::CONDITION_SATISFIED => return true,
                            gl::WAIT_FAILED => {
                                if let Err(err) = self.share.check() {
                                    error!("Error when waiting on fence: {:?}", err);
                                }
                                return false
                            }
                            _ => (),
                        }
                    }
                    // No fence has indicated a postive result
                    false
                };

                // Short-circuit:
                //   Check current state of all fences first,
                //   else go trough each fence and wait til at least one has finished
                waiting(0) || waiting(timeout_ms)
            },
        }
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

    fn destroy_sampler(&mut self, _: n::FatSampler) {
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
