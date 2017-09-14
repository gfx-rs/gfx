use std::ptr;
use std::ops::Range;
use std::rc::Rc;

use gl;
use gl::types::{GLint, GLfloat};
use core::{self as c, device as d, image as i, memory, pass, pso, buffer, mapping};
use core::format::Format;
use std::iter::repeat;

use {Backend as B, Share};
use {conv, device, native as n, state};


fn get_shader_iv(gl: &gl::Gl, name: n::Shader, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetShaderiv(name, query, &mut iv) };
    iv
}

fn get_program_iv(gl: &gl::Gl, name: n::Program, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetProgramiv(name, query, &mut iv) };
    iv
}

fn get_shader_log(gl: &gl::Gl, name: n::Shader) -> String {
    let mut length = get_shader_iv(gl, name, gl::INFO_LOG_LENGTH);
    if length > 0 {
        let mut log = String::with_capacity(length as usize);
        log.extend(repeat('\0').take(length as usize));
        unsafe {
            gl.GetShaderInfoLog(name, length, &mut length,
                (&log[..]).as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as usize);
        log
    } else {
        String::new()
    }
}

pub fn get_program_log(gl: &gl::Gl, name: n::Program) -> String {
    let mut length  = get_program_iv(gl, name, gl::INFO_LOG_LENGTH);
    if length > 0 {
        let mut log = String::with_capacity(length as usize);
        log.extend(repeat('\0').take(length as usize));
        unsafe {
            gl.GetProgramInfoLog(name, length, &mut length,
                (&log[..]).as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as usize);
        log
    } else {
        String::new()
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

impl Device {
    pub fn create_shader_module_from_source(
        &mut self,
        stage: pso::Stage,
        data: &[u8],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        let gl = &self.share.context;

        let target = match stage {
            pso::Stage::Vertex   => gl::VERTEX_SHADER,
            pso::Stage::Hull     => gl::TESS_CONTROL_SHADER,
            pso::Stage::Domain   => gl::TESS_EVALUATION_SHADER,
            pso::Stage::Geometry => gl::GEOMETRY_SHADER,
            pso::Stage::Fragment => gl::FRAGMENT_SHADER,
            pso::Stage::Compute  => gl::COMPUTE_SHADER,
        };

        let name = unsafe { gl.CreateShader(target) };
        unsafe {
            gl.ShaderSource(name, 1,
                &(data.as_ptr() as *const gl::types::GLchar),
                &(data.len() as gl::types::GLint));
            gl.CompileShader(name);
        }
        info!("\tCompiled shader {}", name);

        let status = get_shader_iv(gl, name, gl::COMPILE_STATUS);
        let log = get_shader_log(gl, name);
        if status != 0 {
            if !log.is_empty() {
                warn!("\tLog: {}", log);
            }
            Ok(n::ShaderModule { raw: name })
        } else {
            Err(d::ShaderError::CompilationFailed(String::new())) // TODO
        }
    }

}
impl d::Device<B> for Device {
    fn get_features(&self) -> &c::Features {
        &self.share.features
    }

    fn get_limits(&self) -> &c::Limits {
        &self.share.limits
    }

    fn create_heap(&mut self, _: &c::HeapType, _: d::ResourceHeapType, _: u64) -> Result<n::Heap, d::ResourceHeapError> {
        Ok(n::Heap)
    }

    fn create_renderpass(
        &mut self,
        attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc],
        _dependencies: &[pass::SubpassDependency],
    ) -> n::RenderPass {
        let subpasses =
            subpasses
                .iter()
                .map(|subpass| {
                    let color_attachments =
                        subpass
                            .color_attachments
                            .iter()
                            .map(|&(index, _)| index)
                            .collect();

                    n::SubpassDesc {
                        color_attachments,
                    }
                })
                .collect();

        n::RenderPass {
            attachments: attachments.into(),
            subpasses,
        }
    }

    fn create_pipeline_layout(&mut self, _: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        n::PipelineLayout
    }

    fn create_graphics_pipelines<'a>(
        &mut self,
        descs: &[(pso::GraphicsShaderSet<'a, B>, &n::PipelineLayout, pass::Subpass<'a, B>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        let gl = &self.share.context;
        let priv_caps = &self.share.private_caps;
        descs.iter()
             .map(|&(shaders, layout, subpass, desc)| {
                let subpass = match subpass.main_pass.subpasses.get(subpass.index) {
                    Some(sp) => sp,
                    None => return Err(pso::CreationError::InvalidSubpass(subpass.index)),
                };

                let program = {
                    let name = unsafe { gl.CreateProgram() };

                    let attach_shader = |point_maybe: Option<pso::EntryPoint<B>>| {
                        if let Some(point) = point_maybe {
                            assert_eq!(point.entry, "main");
                            unsafe { gl.AttachShader(name, point.module.raw); }
                        }
                    };

                    // Attach shaders to program
                    attach_shader(Some(shaders.vertex));
                    attach_shader(shaders.hull);
                    attach_shader(shaders.domain);
                    attach_shader(shaders.geometry);
                    attach_shader(shaders.fragment);

                    if !priv_caps.program_interface_supported && priv_caps.frag_data_location_supported {
                        for i in 0..subpass.color_attachments.len() {
                            let color_name = format!("Target{}\0", i);
                            unsafe {
                                gl.BindFragDataLocation(name, i as u32, (&color_name[..]).as_ptr() as *mut gl::types::GLchar);
                            }
                         }
                    }

                    unsafe { gl.LinkProgram(name) };
                    info!("\tLinked program {}", name);

                    let status = get_program_iv(gl, name, gl::LINK_STATUS);
                    let log = get_program_log(gl, name);
                    if status != 0 {
                        if !log.is_empty() {
                            warn!("\tLog: {}", log);
                        }
                    } else {
                        return Err(pso::CreationError::Other);
                    }

                    name
                };

                Ok(n::GraphicsPipeline {
                    program,
                })
             })
             .collect()
    }

    fn create_compute_pipelines<'a>(
        &mut self,
        _descs: &[(pso::EntryPoint<'a, B>, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &mut self,
        _: &n::RenderPass,
        _: &[&n::RenderTargetView],
        _: &[&n::DepthStencilView],
        _: d::Extent,
    ) -> n::FrameBuffer {
        unimplemented!()
    }

    fn create_shader_module(
        &mut self,
        _data: &[u8],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        //TODO: SPIRV loading or conversion to GLSL
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

            gl.SamplerParameterf(name, gl::TEXTURE_MIN_LOD, info.lod_range.start.into());
            gl.SamplerParameterf(name, gl::TEXTURE_MAX_LOD, info.lod_range.end.into());

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

    fn view_buffer_as_constant(&mut self, _: &n::Buffer, _: Range<u64>) -> Result<n::ConstantBufferView, d::TargetViewError> {
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
        n::DescriptorPool { }
    }

    fn create_descriptor_set_layout(&mut self, _: &[pso::DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
        n::DescriptorSetLayout
    }

    fn update_descriptor_sets(&mut self, _: &[pso::DescriptorSetWrite<B>]) {
        unimplemented!()
    }

    fn read_mapping_raw(&mut self, _: &n::Buffer, _: Range<u64>)
        -> Result<(*const u8, Mapping), mapping::Error>
    {
        unimplemented!()
    }

    fn write_mapping_raw(&mut self, _: &n::Buffer, _: Range<u64>)
        -> Result<(*mut u8, Mapping), mapping::Error>
    {
        unimplemented!()
    }

    fn unmap_mapping_raw(&mut self, _: Mapping) {
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
                let mut waiting = |_timeout_ms: u32| {
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

    fn destroy_shader_module(&mut self, _: n::ShaderModule) {
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
