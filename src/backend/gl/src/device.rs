use std::borrow::Borrow;
use std::cell::Cell;
use std::collections::HashMap;
use std::iter::repeat;
use std::ops::Range;
use std::{ptr, mem, slice};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gl;
use gl::types::{GLint, GLenum, GLfloat, GLuint};

use hal::{self as c, device as d, error, image as i, memory, pass, pso, buffer, mapping, query};
use hal::format::{ChannelType, Format, Swizzle};
use hal::pool::CommandPoolCreateFlags;
use hal::queue::QueueFamilyId;
use hal::range::RangeArg;

use spirv_cross::{glsl, spirv, ErrorCode as SpirvErrorCode};

use {Backend as B, Share, Surface, Swapchain};
use {conv, native as n, state};
use info::LegacyFeatures;
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
pub struct UnboundImage {
    raw: GLuint,
    channel: ChannelType,
    requirements: memory::Requirements,
}

/// GL device.
pub struct Device {
    share: Rc<Share>,
}

impl Drop for Device {
    fn drop(&mut self) {
        self.share.open.set(false);
    }
}

impl Device {
    /// Create a new `Device`.
    pub(crate) fn new(share: Rc<Share>) -> Self {
        Device {
            share: share,
        }
    }

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
            Err(d::ShaderError::CompilationFailed(log))
        }
    }

    fn bind_target_compat(gl: &gl::Gl, point: GLenum, attachment: GLenum, view: &n::ImageView) {
        match *view {
            n::ImageView::Surface(surface) => unsafe {
                gl.FramebufferRenderbuffer(point, attachment, gl::RENDERBUFFER, surface);
            },
            n::ImageView::Texture(texture, level) => unsafe {
                gl.BindTexture(gl::TEXTURE_2D, texture);
                gl.FramebufferTexture2D(point, attachment, gl::TEXTURE_2D, texture, level as _);
            },
            n::ImageView::TextureLayer(texture, level, layer) => unsafe {
                gl.BindTexture(gl::TEXTURE_2D, texture);
                gl.FramebufferTexture3D(point, attachment, gl::TEXTURE_2D, texture, level as _, layer as _);
            },
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
    ) -> Result<String, d::ShaderError> {
        let mut compile_options = glsl::CompilerOptions::default();
        // see version table at https://en.wikipedia.org/wiki/OpenGL_Shading_Language
        compile_options.version = match self.share.info.shading_language.tuple() {
            (4, 60) => glsl::Version::V4_60,
            (4, 50) => glsl::Version::V4_50,
            (4, 40) => glsl::Version::V4_40,
            (4, 30) => glsl::Version::V4_30,
            (4, 20) => glsl::Version::V4_20,
            (4, 10) => glsl::Version::V4_10,
            (4, 00) => glsl::Version::V4_00,
            (3, 30) => glsl::Version::V3_30,
            (1, 50) => glsl::Version::V1_50,
            (1, 40) => glsl::Version::V1_40,
            (1, 30) => glsl::Version::V1_30,
            (1, 20) => glsl::Version::V1_20,
            (1, 10) => glsl::Version::V1_10,
            other if other > (4, 60) => glsl::Version::V4_60,
            other => panic!("GLSL version is not recognized: {:?}", other),
        };
        compile_options.vertex.invert_y = true;
        debug!("SPIR-V options {:?}", compile_options);

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

    fn compile_shader(
        &self, point: &pso::EntryPoint<B>, stage: pso::Stage
    ) -> n::Shader {
        assert_eq!(point.entry, "main");
        match *point.module {
            n::ShaderModule::Raw(raw) => raw,
            n::ShaderModule::Spirv(ref spirv) => {
                let mut ast = self.parse_spirv(spirv).unwrap();
                let spirv = self.translate_spirv(&mut ast).unwrap();
                info!("Generated:\n{:?}", spirv);
                match self.create_shader_module_from_source(spirv.as_bytes(), stage).unwrap() {
                    n::ShaderModule::Raw(raw) => raw,
                    _ => panic!("Unhandled")
                }
            }
        }
    }
}

impl d::Device<B> for Device {
    fn allocate_memory(
        &self, _mem_type: c::MemoryTypeId, size: u64,
    ) -> Result<n::Memory, d::OutOfMemory> {
        // TODO
        Ok(n::Memory {
            properties: memory::Properties::CPU_VISIBLE | memory::Properties::CPU_CACHED,
            first_bound_buffer: Cell::new(0),
            size,
        })
    }

    fn create_command_pool(
        &self,
        _family: QueueFamilyId,
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

    fn create_render_pass<'a, IA, IS, ID>(
        &self, attachments: IA, subpasses: IS, _dependencies: ID
    ) -> n::RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        let subpasses =
            subpasses
                .into_iter()
                .map(|subpass| {
                    let color_attachments =
                        subpass
                            .borrow()
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
            attachments: attachments.into_iter().map(|attachment| attachment.borrow().clone()).collect::<Vec<_>>(),
            subpasses,
        }
    }

    fn create_pipeline_layout<IS, IR>(&self, _: IS, _: IR) -> n::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        n::PipelineLayout
    }

    fn create_graphics_pipeline<'a>(
        &self, desc: &pso::GraphicsPipelineDesc<'a, B>
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let gl = &self.share.context;
        let share = &self.share;
        let desc = desc.borrow();
        let subpass = {
            let subpass = desc.subpass;
            match subpass.main_pass.subpasses.get(subpass.index) {
                Some(sp) => sp,
                None => return Err(pso::CreationError::InvalidSubpass(subpass.index)),
            }
        };

        let program = {
            let name = unsafe { gl.CreateProgram() };

            // Attach shaders to program
            let shaders = [
                (pso::Stage::Vertex, Some(&desc.shaders.vertex)),
                (pso::Stage::Hull, desc.shaders.hull.as_ref()),
                (pso::Stage::Domain, desc.shaders.domain.as_ref()),
                (pso::Stage::Geometry, desc.shaders.geometry.as_ref()),
                (pso::Stage::Fragment, desc.shaders.fragment.as_ref()),
            ];

            let shader_names = &shaders
                .iter()
                .filter_map(|&(stage, point_maybe)| {
                    point_maybe.map(|point| {
                        let shader_name = self.compile_shader(point, stage);
                        unsafe { gl.AttachShader(name, shader_name); }
                        shader_name
                    })
                })
                .collect::<Vec<_>>();

            if !share.private_caps.program_interface && share.private_caps.frag_data_location {
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

            for shader_name in shader_names {
                unsafe {
                    gl.DetachShader(name, *shader_name);
                    gl.DeleteShader(*shader_name);
                }
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
            c::Primitive::PatchList(size) => Some(size as _),
            _ => None
        };

        Ok(n::GraphicsPipeline {
            program,
            primitive: conv::primitive_to_gl_primitive(desc.input_assembler.primitive),
            patch_size,
            blend_targets: desc.blender.targets.clone(),
            vertex_buffers: desc.vertex_buffers.clone(),
            attributes: desc.attributes
                .iter()
                .map(|&a| {
                    let (size, format, vertex_attrib_fn) = conv::format_to_gl_format(a.element.format).unwrap();
                    n::AttributeDesc {
                        location: a.location,
                        offset: a.element.offset,
                        binding: a.binding,
                        size,
                        format,
                        vertex_attrib_fn,
                    }
                })
                .collect::<Vec<_>>(),
        })
    }

    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        let gl = &self.share.context;
        let share = &self.share;
        let program = {
            let name = unsafe { gl.CreateProgram() };

            let shader = self.compile_shader(&desc.shader, pso::Stage::Compute);
            unsafe { gl.AttachShader(name, shader) };

            unsafe { gl.LinkProgram(name) };
            info!("\tLinked program {}", name);
            if let Err(err) = share.check() {
                panic!("Error linking program: {:?}", err);
            }

            unsafe {
                gl.DetachShader(name, shader);
                gl.DeleteShader(shader);
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

        Ok(n::ComputePipeline {
            program,
        })
    }

    fn create_framebuffer<I>(
        &self,
        pass: &n::RenderPass,
        attachments: I,
        _extent: d::Extent,
    ) -> Result<n::FrameBuffer, d::FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<n::ImageView>,
    {
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

        let att_points = [
            gl::COLOR_ATTACHMENT0,
            gl::COLOR_ATTACHMENT1,
            gl::COLOR_ATTACHMENT2,
            gl::COLOR_ATTACHMENT3,
        ];

        let mut attachments_len = 0;
        //TODO: exclude depth/stencil attachments from here
        for (&att_point, view) in att_points.iter().zip(attachments.into_iter()) {
            attachments_len += 1;
            if self.share.private_caps.framebuffer_texture {
                Self::bind_target(gl, target, att_point, view.borrow());
            } else {
                Self::bind_target_compat(gl, target, att_point, view.borrow());
            }
        }
        assert_eq!(attachments_len, pass.attachments.len());
        // attachments_len actually equals min(attachments.len(), att_points.len()) until the next assert

        unsafe {
            assert!(pass.attachments.len() <= att_points.len());
            gl.DrawBuffers(attachments_len as _, att_points.as_ptr());
            let status = gl.CheckFramebufferStatus(target);
            assert_eq!(status, gl::FRAMEBUFFER_COMPLETE);
            gl.BindFramebuffer(target, 0);
        }
        if let Err(err) = self.share.check() {
            //TODO: attachments have been consumed
            panic!("Error creating FBO: {:?} for {:?}"/* with attachments {:?}"*/,
               err, pass/*, attachments*/);
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
        if !self.share.legacy_features.contains(LegacyFeatures::SAMPLER_OBJECTS) {
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
                    } else if self.share.features.contains(c::Features::SAMPLER_ANISOTROPY) {
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

            if self.share.legacy_features.contains(LegacyFeatures::SAMPLER_LOD_BIAS) {
                gl.SamplerParameterf(name, gl::TEXTURE_LOD_BIAS, info.lod_bias.into());
            }
            if self.share.legacy_features.contains(LegacyFeatures::SAMPLER_BORDER_COLOR) {
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
        if !self.share.legacy_features.contains(LegacyFeatures::CONSTANT_BUFFER) &&
            usage.contains(buffer::Usage::UNIFORM) {
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
        &self, memory: &n::Memory, offset: u64, unbound: UnboundBuffer,
    ) -> Result<n::Buffer, d::BindError> {
        let gl = &self.share.context;
        let target = unbound.target;

        if offset == 0 {
            memory.first_bound_buffer.set(unbound.name);
        } else {
            assert_ne!(0, memory.first_bound_buffer.get());
        }

        let cpu_can_read = memory.can_download();
        let cpu_can_write = memory.can_upload();

        if self.share.private_caps.buffer_storage {
            //TODO: gl::DYNAMIC_STORAGE_BIT | gl::MAP_PERSISTENT_BIT
            let flags = memory.map_flags();
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
        })
    }

    fn map_memory<R: RangeArg<u64>>(
        &self, memory: &n::Memory, range: R
    ) -> Result<*mut u8, mapping::Error> {
        let gl = &self.share.context;
        let buffer = match memory.first_bound_buffer.get() {
            0 => panic!("No buffer has been bound yet, can't map memory!"),
            other => other,
        };

        assert!(self.share.private_caps.buffer_role_change);
        let target = gl::PIXEL_PACK_BUFFER;
        let access = memory.map_flags();

        let offset = *range.start().unwrap_or(&0);
        let size = *range.end().unwrap_or(&memory.size) - offset;

        let ptr = unsafe {
            gl.BindBuffer(target, buffer);
            let ptr = gl.MapBufferRange(target, offset as _, size as _, access);
            gl.BindBuffer(target, 0);
            ptr as *mut _
        };

        if let Err(err) = self.share.check() {
            panic!("Error mapping memory: {:?} for memory {:?}", err, memory);
        }

        Ok(ptr)
    }

    fn unmap_memory(&self, memory: &n::Memory) {
        let gl = &self.share.context;
        let buffer = match memory.first_bound_buffer.get() {
            0 => panic!("No buffer has been bound yet, can't map memory!"),
            other => other,
        };
        let target = gl::PIXEL_PACK_BUFFER;

        unsafe {
            gl.BindBuffer(target, buffer);
            gl.UnmapBuffer(target);
            gl.BindBuffer(target, 0);
        }

        if let Err(err) = self.share.check() {
            panic!("Error unmapping memory: {:?} for memory {:?}",
                err, memory);
        }
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, _: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        // unimplemented!()
        warn!("memory range invalidation not implemented!");
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, _ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self, _: &n::Buffer, _: Option<Format>, _: R
    ) -> Result<n::BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(
        &self, kind: i::Kind, num_levels: i::Level, format: Format, _: i::Usage
    ) -> Result<UnboundImage, i::CreationError> {
        let gl = &self.share.context;

        let name = unsafe {
            let mut raw = 0;
            gl.GenTextures(1, &mut raw);
            raw
        };

        let int_format = match format {
            Format::Rgba8Unorm => gl::RGBA8,
            Format::Rgba8Srgb => gl::SRGB8_ALPHA8,
            _ => unimplemented!()
        };

        let channel = format.base_format().1;

        let (width, height) = match kind {
            i::Kind::D2(w, h, aa) => unsafe {
                assert_eq!(aa, i::AaMode::Single);
                gl.BindTexture(gl::TEXTURE_2D, name);
                gl.TexStorage2D(gl::TEXTURE_2D, num_levels as _, int_format, w as _, h as _);
                (w, h)
            }
            _ => {
                unimplemented!();
            }
        };

        let surface_desc = format.base_format().0.desc();
        let bytes_per_texel  = surface_desc.bits / 8;

        if let Err(err) = self.share.check() {
            panic!("Error creating image: {:?} for kind {:?} of {:?}",
                err, kind, format);
        }

        Ok(UnboundImage {
            raw: name,
            channel,
            requirements: memory::Requirements {
                size: width as u64 * height as u64 * bytes_per_texel as u64,
                alignment: 1,
                type_mask: 0x7,
            }
        })
    }

    fn get_image_requirements(&self, unbound: &UnboundImage) -> memory::Requirements {
        unbound.requirements
    }

    fn bind_image_memory(&self, _memory: &n::Memory, _offset: u64, image: UnboundImage) -> Result<n::Image, d::BindError> {
        Ok(n::Image {
            kind: n::ImageKind::Texture(image.raw),
            channel: image.channel,
        })
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
        match image.kind {
            n::ImageKind::Surface(surface) => {
                if range.levels.start == 0 && range.layers.start == 0 {
                    Ok(n::ImageView::Surface(surface))
                } else if level != 0 {
                    Err(i::ViewError::Level(level)) //TODO
                } else {
                    Err(i::ViewError::Layer(i::LayerError::OutOfBounds(range.layers)))
                }
            }
            n::ImageKind::Texture(texture) => {
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

    fn create_descriptor_pool<I>(&self, _: usize, _: I) -> n::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        n::DescriptorPool { }
    }

    fn create_descriptor_set_layout<I>(&self, _: I) -> n::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
    {
        n::DescriptorSetLayout
    }

    fn write_descriptor_sets<'a, I, R>(&self, writes: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetWrite<'a, B, R>>,
        R: 'a + RangeArg<u64>,
    {
        for _write in writes {
            //unimplemented!() // not panicing because of Warden
            error!("TODO: implement `write_descriptor_sets`");
        }
    }

    fn copy_descriptor_sets<'a, I>(&self, copies: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, B>>,
    {
        for _copy in copies {
            unimplemented!()
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

    fn reset_fences<I>(&self, fences: I)
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        if !self.share.private_caps.sync {
            return
        }

        let gl = &self.share.context;
        for fence in fences {
            let fence = fence.borrow();
            let sync = fence.0.get();
            unsafe {
                if gl.IsSync(sync) == gl::TRUE {
                    gl.DeleteSync(sync);
                }
            }
            fence.0.set(ptr::null())
        }
    }

    fn wait_for_fence(&self, fence: &n::Fence, timeout_ms: u32) -> bool {
        if !self.share.private_caps.sync {
            return true;
        }
        match wait_fence(fence, &self.share.context, timeout_ms) {
            gl::TIMEOUT_EXPIRED => false,
            gl::WAIT_FAILED => {
                if let Err(err) = self.share.check() {
                    error!("Error when waiting on fence: {:?}", err);
                }
                false
            }
            _ => true,
        }
    }

    fn get_fence_status(&self, _: &n::Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, _memory: n::Memory) {
        // nothing to do
    }

    fn create_query_pool(&self, _ty: query::QueryType, _count: u32) -> () {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_shader_module(&self, _: n::ShaderModule) {
        // Assumes compiled shaders are managed internally
    }

    fn destroy_render_pass(&self, _: n::RenderPass) {
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

    fn destroy_buffer(&self, buffer: n::Buffer) {
        unsafe {
            self.share.context.DeleteBuffers(1, &buffer.raw);
        }
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

    fn destroy_fence(&self, fence: n::Fence) {
        unsafe {
            self.share.context.DeleteSync(fence.0.get());
        }
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

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unsafe { self.share.context.Finish(); }
        Ok(())
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
