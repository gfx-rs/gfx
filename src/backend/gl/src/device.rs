use std::ptr;
use std::ops::Range;
use std::rc::Rc;

use gl;
use gl::types::{GLint, GLenum, GLfloat};
use core::{self as c, device as d, image as i, memory, pass, pso, buffer, mapping};
use core::format::Format;
use std::iter::repeat;

use {Backend as B, Share};
use {conv, native as n, state};


fn get_shader_iv(gl: &gl::Gl, name: n::Shader, query: GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetShaderiv(name, query, &mut iv) };
    iv
}

fn get_program_iv(gl: &gl::Gl, name: n::Program, query: GLenum) -> gl::types::GLint {
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
pub struct UnboundBuffer {
    name: n::RawBuffer,
    target: GLenum,
    requirements: memory::Requirements,
}

#[derive(Debug)]
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

impl Device {
    pub fn create_shader_module_from_source(
        &mut self,
        data: &[u8],
        stage: pso::Stage,
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
        if let Err(err) = self.share.check() {
            panic!("Error compiling shader: {:?}", err);
        }

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

    fn bind_target(gl: &gl::Gl, point: GLenum, attachment: GLenum, view: &n::TargetView) {
        match *view {
            n::TargetView::Surface(surface) => unsafe {
                gl.FramebufferRenderbuffer(point, attachment, gl::RENDERBUFFER, surface);
            },
            n::TargetView::Texture(texture, level) => unsafe {
                gl.FramebufferTexture(point, attachment, texture, level as _);
            },
            n::TargetView::TextureLayer(texture, level, layer) => unsafe {
                gl.FramebufferTextureLayer(point, attachment, texture, level as _, layer as _);
            },
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

    fn allocate_memory(
        &mut self, mem_type: &c::MemoryType, _size: u64,
    ) -> Result<n::Memory, d::OutOfMemory> {
        Ok(n::Memory {
            properties: mem_type.properties,
        })
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
        let share = &self.share;
        descs.iter()
             .map(|&(shaders, _layout, subpass, _desc)| {
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

                    if !priv_caps.program_interface && priv_caps.frag_data_location {
                        for i in 0..subpass.color_attachments.len() {
                            let color_name = format!("Target{}\0", i);
                            unsafe {
                                gl.BindFragDataLocation(name, i as u32, (&color_name[..]).as_ptr() as *mut gl::types::GLchar);
                            }
                         }
                    }

                    unsafe { gl.LinkProgram(name) };
                    info!("\tLinked program {}", name);
                    if let Err(err) = share.check() {
                        panic!("Error linking program: {:?}", err);
                    }

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
        pass: &n::RenderPass,
        rtvs: &[&n::TargetView],
        dsvs: &[&n::TargetView],
        _extent: d::Extent,
    ) -> Result<n::FrameBuffer, d::FramebufferError> {
        if !self.share.private_caps.framebuffer {
            return Err(d::FramebufferError);
        }

        let gl = &self.share.context;
        let target = gl::DRAW_FRAMEBUFFER;
        let mut name = 0;
        unsafe {
            gl.GenFramebuffers(1, &mut name);
            gl.BindFramebuffer(target, name);
        }

        assert_eq!(rtvs.len() + dsvs.len(), pass.attachments.len());
        for (i, view) in rtvs.iter().enumerate() {
            let attachment = gl::COLOR_ATTACHMENT0 + i as GLenum;
            Self::bind_target(gl, target, attachment, view);
        }

        match dsvs.len() {
            0 => (),
            1 => {
                Self::bind_target(gl, target, gl::DEPTH_STENCIL_ATTACHMENT, &dsvs[0]);
            },
            2 => {
                Self::bind_target(gl, target, gl::DEPTH_ATTACHMENT, &dsvs[0]);
                Self::bind_target(gl, target, gl::STENCIL_ATTACHMENT, &dsvs[1]);
            },
            _ => panic!("Unexpected depth/stencil attachments {:?}", dsvs),
        }

        unsafe {
            let status = gl.CheckFramebufferStatus(target);
            assert_eq!(status, gl::FRAMEBUFFER_COMPLETE);
            gl.BindFramebuffer(target, 0);
        }
        if let Err(err) = self.share.check() {
            panic!("Error creating FBO: {:?} for {:?}, rtvs {:?}, dsvs {:?}", err, pass, rtvs, dsvs);
        }

        Ok(name)
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

    fn create_buffer(
        &mut self, size: u64, stride: u64, usage: buffer::Usage,
    ) -> Result<UnboundBuffer, buffer::CreationError> {
        if !self.share.features.constant_buffer && usage.contains(buffer::CONSTANT) {
            error!("Constant buffers are not supported by this GL version");
            return Err(buffer::CreationError::Other);
        }

        let target = if self.share.private_caps.buffer_role_change {
            gl::ARRAY_BUFFER
        } else {
            match conv::buffer_usage_to_gl_target(usage) {
                Some(target) => target,
                None => return Err(buffer::CreationError::Usage(usage)),
            }
        };

        let gl = &self.share.context;
        let mut name = 0;
        unsafe {
            gl.GenBuffers(1, &mut name);
        }

        Ok(UnboundBuffer {
            name,
            target,
            requirements: memory::Requirements {
                size,
                alignment: stride,
                type_mask: 0x7,
            },
        })
    }

    fn get_buffer_requirements(&mut self, unbound: &UnboundBuffer) -> memory::Requirements {
        unbound.requirements
    }

    fn bind_buffer_memory(
        &mut self, memory: &n::Memory, _offset: u64, unbound: UnboundBuffer,
    ) -> Result<n::Buffer, d::BindError> {
        let gl = &self.share.context;
        let target = unbound.target;

        let cpu_can_read = memory.can_download();
        let cpu_can_write = memory.can_upload();

        if self.share.private_caps.buffer_storage {
            //TODO: gl::DYNAMIC_STORAGE_BIT | gl::MAP_PERSISTENT_BIT
            let mut flags = 0;
            if cpu_can_read {
                flags |= gl::MAP_READ_BIT;
            }
            if cpu_can_write {
                flags |= gl::MAP_WRITE_BIT;
            }
            //TODO: use *Named calls to avoid binding
            unsafe {
                gl.BindBuffer(target, unbound.name);
                gl.BufferStorage(target,
                    unbound.requirements.size as _,
                    ptr::null(),
                    flags,
                );
                gl.BindBuffer(target, 0);
            }
        }
        else {
            let flags = if cpu_can_read && cpu_can_write {
                gl::DYNAMIC_DRAW
            } else if cpu_can_write {
                gl::STREAM_DRAW
            } else if cpu_can_read {
                gl::STREAM_READ
            } else {
                gl::STATIC_DRAW
            };
            unsafe {
                gl.BindBuffer(target, unbound.name);
                gl.BufferData(target,
                    unbound.requirements.size as _,
                    ptr::null(),
                    flags,
                );
                gl.BindBuffer(target, 0);
            }
        }

        if let Err(err) = self.share.check() {
            panic!("Error {:?} initializing buffer {:?}, memory {:?}",
                err, unbound, memory.properties);
        }

        Ok(n::Buffer {
            raw: unbound.name,
            target,
            cpu_can_read,
            cpu_can_write,
        })
    }

    fn create_image(&mut self, _: i::Kind, _: i::Level, _: Format, _: i::Usage)
         -> Result<UnboundImage, i::CreationError>
    {
        unimplemented!()
    }

    fn get_image_requirements(&mut self, _: &UnboundImage) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_image_memory(&mut self, _: &n::Memory, _: u64, _: UnboundImage) -> Result<n::Image, d::BindError> {
        unimplemented!()
    }

    fn view_buffer_as_constant(&mut self, _: &n::Buffer, _: Range<u64>) -> Result<n::ConstantBufferView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self,
        image: &n::Image, _format: Format, (level, layers): i::SubresourceLayers,
    ) -> Result<n::TargetView, d::TargetViewError> {
        //TODO: check if `layers.end` covers all the layers
        //TODO: check format
        match *image {
            n::Image::Surface(surface) => {
                if level == 0 && layers.start == 0 {
                    Ok(n::TargetView::Surface(surface))
                } else if level != 0 {
                    Err(d::TargetViewError::Level(level)) //TODO
                } else {
                    Err(d::TargetViewError::Layer(i::LayerError::OutOfBounds(layers)))
                }
            },
            n::Image::Texture(texture) => {
                //TODO: check that `level` exists
                if layers.start == 0 {
                    Ok(n::TargetView::Texture(texture, level))
                } else if layers.start + 1 == layers.end {
                    Ok(n::TargetView::TextureLayer(texture, level, layers.start))
                } else {
                    Err(d::TargetViewError::Layer(i::LayerError::OutOfBounds(layers)))
                }
            },
        }
    }

    fn view_image_as_depth_stencil(&mut self,
        image: &n::Image, format: Format, layers: i::SubresourceLayers,
    ) -> Result<n::TargetView, d::TargetViewError> {
        self.view_image_as_render_target(image, format, layers) //TODO
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

    fn acquire_mapping_raw(&mut self, buffer: &n::Buffer, read: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error>
    {
        let access = match read {
            Some(_) if buffer.cpu_can_read && buffer.cpu_can_write => gl::READ_WRITE,
            Some(_) if buffer.cpu_can_read => gl::READ_ONLY,
            None if buffer.cpu_can_write => gl::WRITE_ONLY,
            _ => return Err(mapping::Error::InvalidAccess)
        };
        let gl = &self.share.context;

        let data = unsafe {
            gl.BindBuffer(buffer.target, buffer.raw);
            let ptr = gl.MapBuffer(buffer.target, access);
            gl.BindBuffer(buffer.target, 0);
            ptr
        };

        if let Err(err) = self.share.check() {
            panic!("Error mapping buffer: {:?}, {:?}, access = {}", err, buffer, access);
        }
        debug_assert_ne!(data, ptr::null_mut());
        Ok(data as _)
    }

    fn release_mapping_raw(&mut self, buffer: &n::Buffer, wrote: Option<Range<u64>>) {
        assert!(wrote.is_none() || buffer.cpu_can_write);
        let gl = &self.share.context;
        unsafe {
            gl.BindBuffer(buffer.target, buffer.raw);
            gl.UnmapBuffer(buffer.target);
            gl.BindBuffer(buffer.target, 0);
        }
    }

    fn create_semaphore(&mut self) -> n::Semaphore {
        n::Semaphore
    }

    fn create_fence(&mut self, signalled: bool) -> n::Fence {
        let sync = if signalled && self.share.private_caps.sync {
            let gl = &self.share.context;
            unsafe { gl.FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0) }
        } else {
            ptr::null()
        };
        n::Fence::new(sync)
    }

    fn reset_fences(&mut self, fences: &[&n::Fence]) {
        if !self.share.private_caps.sync {
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
        if !self.share.private_caps.sync {
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
                let waiting = |_timeout_ms: u32| {
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
                    // No fence has indicated a positive result
                    false
                };

                // Short-circuit:
                //   Check current state of all fences first,
                //   else go trough each fence and wait till at least one has finished
                waiting(0) || waiting(timeout_ms)
            },
        }
    }

    fn free_memory(&mut self, _: n::Memory) {
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

    fn destroy_render_target_view(&mut self, _: n::TargetView) {
        unimplemented!()
    }

    fn destroy_depth_stencil_view(&mut self, _: n::TargetView) {
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

pub fn wait_fence(fence: &n::Fence, gl: &gl::Gl, timeout_ms: u32) -> GLenum {
    let timeout = timeout_ms as u64 * 1_000_000;
    // TODO:
    // This can be called by multiple objects wanting to ensure they have exclusive
    // access to a resource. How much does this call costs ? The status of the fence
    // could be cached to avoid calling this more than once (in core or in the backend ?).
    unsafe { gl.ClientWaitSync(fence.0.get(), gl::SYNC_FLUSH_COMMANDS_BIT, timeout) }
}
