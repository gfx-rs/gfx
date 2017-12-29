use std::collections::HashMap;
use std::iter::repeat;
use std::ops::Range;
use std::{ptr, mem, slice};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gl;
use gl::types::{GLint, GLenum, GLfloat};
use hal::{self as c, device as d, image as i, memory, pass, pso, buffer, mapping, query, Primitive};
use hal::format::{Format, Swizzle};
use hal::pool::CommandPoolCreateFlags;
use spirv_cross::{glsl, spirv, ErrorCode as SpirvErrorCode};

use {Backend as B, QueueFamily, Share, Surface, Swapchain};
use {conv, native as n, state};
use pool::{BufferMemory, OwnedBuffer, RawCommandPool};

/// Emit error during shader module creation. Used if we don't expect an error
/// but might panic due to an exception in SPIRV-Cross.
fn gen_unexpected_error(err: SpirvErrorCode) -> d::ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unexpected error".into(),
    };
    d::ShaderError::CompilationFailed(msg)
}

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

fn create_fbo_internal(gl: &gl::Gl) -> gl::types::GLuint {
    let mut name = 0 as n::FrameBuffer;
    unsafe {
        gl.GenFramebuffers(1, &mut name);
    }
    info!("\tCreated frame buffer {}", name);
    name
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
        &self,
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
            Ok(n::ShaderModule::Raw(name))
        } else {
            Err(d::ShaderError::CompilationFailed(String::new())) // TODO
        }
    }

    fn bind_target(gl: &gl::Gl, point: GLenum, attachment: GLenum, view: &n::ImageView) {
        match *view {
            n::ImageView::Surface(surface) => unsafe {
                gl.FramebufferRenderbuffer(point, attachment, gl::RENDERBUFFER, surface);
            },
            n::ImageView::Texture(texture, level) => unsafe {
                gl.FramebufferTexture(point, attachment, texture, level as _);
            },
            n::ImageView::TextureLayer(texture, level, layer) => unsafe {
                gl.FramebufferTextureLayer(point, attachment, texture, level as _, layer as _);
            },
        }
    }

    fn parse_spirv(&self, raw_data: &[u8]) -> Result<spirv::Ast<glsl::Target>, d::ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(raw_data.len() & 3, 0);

        let module = spirv::Module::from_words(unsafe {
            slice::from_raw_parts(
                raw_data.as_ptr() as *const u32,
                raw_data.len() / mem::size_of::<u32>(),
            )
        });

        spirv::Ast::parse(&module)
            .map_err(|err| {
                let msg =  match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown parsing error".into(),
                };
                d::ShaderError::CompilationFailed(msg)
            })
    }

    fn translate_spirv(
        &self,
        ast: &mut spirv::Ast<glsl::Target>,
        version: glsl::Version,
    ) -> Result<String, d::ShaderError> {
        let mut compile_options = glsl::CompilerOptions::default();
        compile_options.version = version;
        compile_options.vertex.invert_y = true;

        ast.set_compiler_options(&compile_options)
            .map_err(gen_unexpected_error)?;
        ast.compile()
            .map_err(|err| {
                let msg =  match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                };
                d::ShaderError::CompilationFailed(msg)
            })
    }
}

impl d::Device<B> for Device {
    fn allocate_memory(
        &self, _mem_type: c::MemoryTypeId, _size: u64,
    ) -> Result<n::Memory, d::OutOfMemory> {
        unimplemented!()
    }

    fn create_command_pool(
        &self,
        _family: &QueueFamily,
        flags: CommandPoolCreateFlags,
    ) -> RawCommandPool {
        let fbo = create_fbo_internal(&self.share.context);
        let limits = self.share.limits.into();
        let memory = if flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
            BufferMemory::Individual {
                storage: HashMap::new(),
                next_buffer_id: 0,
            }
        } else {
            BufferMemory::Linear(OwnedBuffer::new())
        };

        // Ignoring `TRANSIENT` hint, unsure how to make use of this.

        RawCommandPool {
            fbo,
            limits,
            memory: Arc::new(Mutex::new(memory)),
        }
    }

    fn destroy_command_pool(&self, pool: RawCommandPool) {
        let gl = &self.share.context;
        unsafe {
            gl.DeleteFramebuffers(1, &pool.fbo);
        }
    }

    fn create_render_pass(
        &self,
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
                            .colors
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

    fn create_pipeline_layout(&self, _: &[&n::DescriptorSetLayout], _: &[(pso::ShaderStageFlags, Range<u32>)]) -> n::PipelineLayout {
        n::PipelineLayout
    }

    fn create_graphics_pipelines<'a>(
        &self,
        descs: &[pso::GraphicsPipelineDesc<'a, B>],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        let gl = &self.share.context;
        let priv_caps = &self.share.private_caps;
        let share = &self.share;
        descs.iter()
             .map(|desc| {
                let subpass = {
                    let subpass = desc.subpass;
                    match subpass.main_pass.subpasses.get(subpass.index) {
                        Some(sp) => sp,
                        None => return Err(pso::CreationError::InvalidSubpass(subpass.index)),
                    }
                };

                let program = {
                    let name = unsafe { gl.CreateProgram() };

                    let attach_shader = |point_maybe: Option<&pso::EntryPoint<B>>, stage: pso::Stage| {
                        if let Some(point) = point_maybe {
                            assert_eq!(point.entry, "main");
                            let raw = match *point.module {
                                n::ShaderModule::Raw(raw) => raw,
                                n::ShaderModule::Spirv(ref spirv) => {
                                    let mut ast = self.parse_spirv(spirv).unwrap();
                                    let spirv = self.translate_spirv(&mut ast, glsl::Version::V4_10).unwrap();
                                    info!("Generated:\n{:?}", spirv);
                                    match self.create_shader_module_from_source(spirv.as_bytes(), stage).unwrap() {
                                        n::ShaderModule::Raw(raw) => raw,
                                        _ => panic!("Unhandled")
                                    }
                                }
                            };
                            unsafe { gl.AttachShader(name, raw); }
                        }
                    };

                    // Attach shaders to program
                    attach_shader(Some(&desc.shaders.vertex), pso::Stage::Vertex);
                    attach_shader(desc.shaders.hull.as_ref(), pso::Stage::Hull);
                    attach_shader(desc.shaders.domain.as_ref(), pso::Stage::Domain);
                    attach_shader(desc.shaders.geometry.as_ref(), pso::Stage::Geometry);
                    attach_shader(desc.shaders.fragment.as_ref(), pso::Stage::Fragment);

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

                let patch_size = match desc.input_assembler.primitive {
                    Primitive::PatchList(size) => Some(size as _),
                    _ => None
                };

                Ok(n::GraphicsPipeline {
                    program,
                    primitive: conv::primitive_to_gl_primitive(desc.input_assembler.primitive),
                    patch_size,
                    blend_targets: desc.blender.targets.clone(),
                })
             })
             .collect()
    }

    fn create_compute_pipelines<'a>(
        &self,
        _descs: &[pso::ComputePipelineDesc<'a, B>],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &self,
        pass: &n::RenderPass,
        attachments: &[&n::ImageView],
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

        assert_eq!(attachments.len(), pass.attachments.len());
        //TODO: exclude depth/stencil attachments from here
        for (i, view) in attachments.iter().enumerate() {
            let attachment = gl::COLOR_ATTACHMENT0 + i as GLenum;
            Self::bind_target(gl, target, attachment, view);
        }

        unsafe {
            let status = gl.CheckFramebufferStatus(target);
            assert_eq!(status, gl::FRAMEBUFFER_COMPLETE);
            gl.BindFramebuffer(target, 0);
        }
        if let Err(err) = self.share.check() {
            panic!("Error creating FBO: {:?} for {:?} with attachments {:?}", err, pass, attachments);
        }

        Ok(name)
    }

    fn create_shader_module(
        &self,
        raw_data: &[u8],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        Ok(n::ShaderModule::Spirv(raw_data.into()))
    }

    fn create_sampler(&self, info: i::SamplerInfo) -> n::FatSampler {
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
        &self, size: u64, usage: buffer::Usage,
    ) -> Result<UnboundBuffer, buffer::CreationError> {
        if !self.share.features.constant_buffer && usage.contains(buffer::Usage::UNIFORM) {
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
                alignment: 1, // TODO: do we need specific alignment for any use-case?
                type_mask: 0x7,
            },
        })
    }

    fn get_buffer_requirements(&self, unbound: &UnboundBuffer) -> memory::Requirements {
        unbound.requirements
    }

    fn bind_buffer_memory(
        &self, memory: &n::Memory, _offset: u64, unbound: UnboundBuffer,
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

    fn create_buffer_view(
        &self, _: &n::Buffer, _: Option<Format>, _: Range<u64>
    ) -> Result<n::BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(&self, _: i::Kind, _: i::Level, _: Format, _: i::Usage)
         -> Result<UnboundImage, i::CreationError>
    {
        unimplemented!()
    }

    fn get_image_requirements(&self, _: &UnboundImage) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_image_memory(&self, _: &n::Memory, _: u64, _: UnboundImage) -> Result<n::Image, d::BindError> {
        unimplemented!()
    }

    fn create_image_view(&self,
        image: &n::Image, _format: Format, swizzle: Swizzle, range: i::SubresourceRange,
    ) -> Result<n::ImageView, i::ViewError> {
        //TODO: check if `layers.end` covers all the layers
        let level = range.levels.start;
        assert_eq!(level + 1, range.levels.end);
        //assert_eq!(format, image.format);
        assert_eq!(swizzle, Swizzle::NO);
        //TODO: check format
        match *image {
            n::Image::Surface(surface) => {
                if range.levels.start == 0 && range.layers.start == 0 {
                    Ok(n::ImageView::Surface(surface))
                } else if level != 0 {
                    Err(i::ViewError::Level(level)) //TODO
                } else {
                    Err(i::ViewError::Layer(i::LayerError::OutOfBounds(range.layers)))
                }
            }
            n::Image::Texture(texture) => {
                //TODO: check that `level` exists
                if range.layers.start == 0 {
                    Ok(n::ImageView::Texture(texture, level))
                } else if range.layers.start + 1 == range.layers.end {
                    Ok(n::ImageView::TextureLayer(texture, level, range.layers.start))
                } else {
                    Err(i::ViewError::Layer(i::LayerError::OutOfBounds(range.layers)))
                }
            }
        }
    }

    fn create_descriptor_pool(&self, _: usize, _: &[pso::DescriptorRangeDesc]) -> n::DescriptorPool {
        n::DescriptorPool { }
    }

    fn create_descriptor_set_layout(&self, _: &[pso::DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
        n::DescriptorSetLayout
    }

    fn update_descriptor_sets(&self, _: &[pso::DescriptorSetWrite<B>]) {
        unimplemented!()
    }

    fn acquire_mapping_raw(&self, buffer: &n::Buffer, read: Option<Range<u64>>)
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

    fn release_mapping_raw(&self, buffer: &n::Buffer, wrote: Option<Range<u64>>) {
        assert!(wrote.is_none() || buffer.cpu_can_write);
        let gl = &self.share.context;
        unsafe {
            gl.BindBuffer(buffer.target, buffer.raw);
            gl.UnmapBuffer(buffer.target);
            gl.BindBuffer(buffer.target, 0);
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        n::Semaphore
    }

    fn create_fence(&self, signalled: bool) -> n::Fence {
        let sync = if signalled && self.share.private_caps.sync {
            let gl = &self.share.context;
            unsafe { gl.FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0) }
        } else {
            ptr::null()
        };
        n::Fence::new(sync)
    }

    fn reset_fences(&self, fences: &[&n::Fence]) {
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

    fn wait_for_fences(&self, fences: &[&n::Fence], wait: d::WaitFor, timeout_ms: u32) -> bool {
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

    fn get_fence_status(&self, _: &n::Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, _: n::Memory) {
        unimplemented!()
    }

    fn create_query_pool(&self, _ty: query::QueryType, _count: u32) -> () {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_shader_module(&self, _: n::ShaderModule) {
        unimplemented!()
    }

    fn destroy_renderpass(&self, _: n::RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&self, _: n::PipelineLayout) {
        unimplemented!()
    }
    fn destroy_graphics_pipeline(&self, _: n::GraphicsPipeline) {
        unimplemented!()
    }
    fn destroy_compute_pipeline(&self, _: n::ComputePipeline) {
        unimplemented!()
    }
    fn destroy_framebuffer(&self, _: n::FrameBuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&self, _: n::Buffer) {
        unimplemented!()
    }
    fn destroy_buffer_view(&self, _: n::BufferView) {
        unimplemented!()
    }
    fn destroy_image(&self, _: n::Image) {
        unimplemented!()
    }
    fn destroy_image_view(&self, _: n::ImageView) {
        unimplemented!()
    }
    fn destroy_sampler(&self, _: n::FatSampler) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&self, _: n::DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&self, _: n::DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&self, _: n::Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&self, _: n::Semaphore) {
        unimplemented!()
    }

    fn create_swapchain(
        &self,
        surface: &mut Surface,
        config: c::SwapchainConfig,
    ) -> (Swapchain, c::Backbuffer<B>) {
        self.create_swapchain_impl(surface, config)
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
