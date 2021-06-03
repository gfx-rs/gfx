use crate::{
    command as cmd, conv,
    info::LegacyFeatures,
    native as n,
    pool::{BufferMemory, CommandPool, OwnedBuffer},
    state, Backend as B, FastHashMap, GlContainer, GlContext, MemoryUsage, Share, Starc,
    MAX_TEXTURE_SLOTS,
};

use hal::{
    buffer, device as d,
    format::{ChannelType, Format, Swizzle},
    image as i, memory, pass,
    pool::CommandPoolCreateFlags,
    pso, query, queue,
};

use glow::HasContext;
use parking_lot::Mutex;

use std::{ops::Range, slice, sync::Arc};

#[cfg(feature = "cross")]
type CrossAst = spirv_cross::spirv::Ast<spirv_cross::glsl::Target>;

fn create_fbo_internal(
    share: &Starc<Share>,
) -> Option<<GlContext as glow::HasContext>::Framebuffer> {
    if share.private_caps.framebuffer {
        let gl = &share.context;
        let name = unsafe { gl.create_framebuffer() }.unwrap();
        log::info!("\tCreated frame buffer {:?}", name);
        Some(name)
    } else {
        None
    }
}

struct CompilationContext<'a> {
    layout: &'a n::PipelineLayout,
    sampler_map: &'a mut n::SamplerBindMap,
    name_binding_map: &'a mut FastHashMap<String, (n::BindingRegister, u8)>,
}

impl<'a> CompilationContext<'a> {
    fn reborrow(&mut self) -> CompilationContext<'_> {
        CompilationContext {
            layout: self.layout,
            sampler_map: self.sampler_map,
            name_binding_map: self.name_binding_map,
        }
    }
}

/// GL device.
#[derive(Debug)]
pub struct Device {
    pub(crate) share: Starc<Share>,
    features: hal::Features,
    #[cfg(feature = "cross")]
    spv_options: naga::back::spv::Options,
}

impl Drop for Device {
    fn drop(&mut self) {
        self.share.open.set(false);
    }
}

impl Device {
    /// Create a new `Device`.
    pub(crate) fn new(share: Starc<Share>, features: hal::Features) -> Self {
        Device {
            share: share,
            features,
            #[cfg(feature = "cross")]
            spv_options: {
                use naga::back::spv;
                let mut flags = spv::WriterFlags::empty();
                flags.set(spv::WriterFlags::DEBUG, cfg!(debug_assertions));
                flags.set(
                    spv::WriterFlags::ADJUST_COORDINATE_SPACE,
                    !features.contains(hal::Features::NDC_Y_UP),
                );
                spv::Options {
                    lang_version: (1, 0),
                    flags,
                    // doesn't matter since we send it through SPIRV-Cross
                    capabilities: None,
                }
            },
        }
    }

    fn create_shader_module_raw(
        gl: &GlContainer,
        shader: &str,
        stage: naga::ShaderStage,
    ) -> Result<n::Shader, d::ShaderError> {
        let target = match stage {
            naga::ShaderStage::Vertex => glow::VERTEX_SHADER,
            naga::ShaderStage::Fragment => glow::FRAGMENT_SHADER,
            naga::ShaderStage::Compute => glow::COMPUTE_SHADER,
        };

        let name = unsafe { gl.create_shader(target) }.unwrap();
        unsafe {
            gl.shader_source(name, shader);
            gl.compile_shader(name);
        }
        log::info!("\tCompiled shader {:?}", name);
        if cfg!(debug_assertions) {
            let err = super::Error::from_error_code(unsafe { gl.get_error() });
            assert_eq!(err, super::Error::NoError, "Error compiling shader");
        }

        let compiled_ok = unsafe { gl.get_shader_compile_status(name) };
        let log = unsafe { gl.get_shader_info_log(name) };
        if compiled_ok {
            if !log.is_empty() {
                log::warn!("\tLog: {}", log);
            }
            Ok(name)
        } else {
            Err(d::ShaderError::CompilationFailed(log))
        }
    }

    fn create_shader_program(
        &self,
        shaders: &[(naga::ShaderStage, Option<&pso::EntryPoint<B>>)],
        layout: &n::PipelineLayout,
    ) -> Result<(glow::Program, n::SamplerBindMap), pso::CreationError> {
        let gl = &self.share.context;
        let program = unsafe { gl.create_program().unwrap() };

        let mut name_binding_map = FastHashMap::<String, (n::BindingRegister, u8)>::default();
        let mut sampler_map = [None; MAX_TEXTURE_SLOTS];

        let mut has_vertex_stage = false;
        let mut has_fragment_stage = false;
        let mut context = CompilationContext {
            layout,
            sampler_map: &mut sampler_map,
            name_binding_map: &mut name_binding_map,
        };

        let mut shaders_to_delete = arrayvec::ArrayVec::<[_; 3]>::new();

        for &(stage, point_maybe) in shaders {
            if let Some(point) = point_maybe {
                match stage {
                    naga::ShaderStage::Vertex => has_vertex_stage = true,
                    naga::ShaderStage::Fragment => has_fragment_stage = true,
                    naga::ShaderStage::Compute => (),
                }

                let shader = self
                    .compile_shader(point, stage, context.reborrow())
                    .map_err(|err| {
                        let error = format!("{} shader compilation failed: {:?}", err, stage);
                        pso::CreationError::ShaderCreationError(stage.into(), error)
                    })?;
                unsafe {
                    gl.attach_shader(program, shader);
                    shaders_to_delete.push(shader);
                }
            }
        }

        // Create empty fragment shader if only vertex shader is present
        if has_vertex_stage && !has_fragment_stage {
            let sl = &self.share.info.shading_language;
            let version = (sl.major * 100 + sl.minor * 10) as u16;
            let shader_type = if sl.is_embedded { "es" } else { "" };
            let shader_src = format!(
                "#version {version} {shader_type} \n void main(void) {{}}",
                version = version,
                shader_type = shader_type
            );
            log::debug!(
                "Only vertex shader is present. Creating empty fragment shader:\n{}",
                shader_src
            );
            let shader = Self::create_shader_module_raw(
                &self.share.context,
                &shader_src,
                naga::ShaderStage::Fragment,
            )
            .unwrap();
            unsafe {
                gl.attach_shader(program, shader);
                shaders_to_delete.push(shader);
            }
        }

        unsafe {
            gl.link_program(program);
        }

        for shader in shaders_to_delete {
            unsafe {
                gl.delete_shader(shader);
            }
        }

        log::info!("\tLinked program {:?}", program);
        if let Err(err) = self.share.check() {
            panic!("Error linking program: {:?}", err);
        }

        let linked_ok = unsafe { gl.get_program_link_status(program) };
        let log = unsafe { gl.get_program_info_log(program) };
        if !linked_ok {
            let error = format!("Program {:?} linking error:{}", program, log);
            return Err(pso::CreationError::ShaderCreationError(
                pso::ShaderStageFlags::GRAPHICS,
                error,
            ));
        }
        if !log.is_empty() {
            log::warn!("\tLog: {}", log);
        }

        if !self
            .share
            .legacy_features
            .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
        {
            unsafe {
                gl.use_program(Some(program));
            }
            for (name, &(register, slot)) in name_binding_map.iter() {
                log::trace!("Get binding {:?} from program {:?}", name, program);
                match register {
                    n::BindingRegister::Textures => unsafe {
                        let loc = gl.get_uniform_location(program, name).unwrap();
                        gl.uniform_1_i32(Some(&loc), slot as _);
                    },
                    n::BindingRegister::UniformBuffers => unsafe {
                        let index = gl.get_uniform_block_index(program, name).unwrap();
                        gl.uniform_block_binding(program, index, slot as _);
                    },
                    n::BindingRegister::StorageBuffers => unsafe {
                        let index = gl.get_shader_storage_block_index(program, name).unwrap();
                        gl.shader_storage_block_binding(program, index, slot as _);
                    },
                }
            }
        }

        Ok((program, sampler_map))
    }

    fn _bind_target_compat(gl: &GlContainer, point: u32, attachment: u32, view: &n::ImageView) {
        match *view {
            n::ImageView::Renderbuffer { raw: rb, .. } => unsafe {
                gl.framebuffer_renderbuffer(point, attachment, glow::RENDERBUFFER, Some(rb));
            },
            n::ImageView::Texture {
                target,
                raw,
                ref sub,
                is_3d: false,
            } => unsafe {
                gl.bind_texture(target, Some(raw));
                gl.framebuffer_texture_2d(
                    point,
                    attachment,
                    target,
                    Some(raw),
                    sub.level_start as _,
                );
            },
            n::ImageView::Texture {
                target,
                raw,
                ref sub,
                is_3d: true,
            } => unsafe {
                gl.bind_texture(target, Some(raw));
                gl.framebuffer_texture_3d(
                    point,
                    attachment,
                    target,
                    Some(raw),
                    sub.level_start as _,
                    sub.layer_start as _,
                );
            },
        }
    }

    pub(crate) fn bind_target(gl: &GlContainer, point: u32, attachment: u32, view: &n::ImageView) {
        match *view {
            n::ImageView::Renderbuffer { raw: rb, .. } => unsafe {
                gl.framebuffer_renderbuffer(point, attachment, glow::RENDERBUFFER, Some(rb));
            },
            n::ImageView::Texture {
                target: _,
                raw,
                ref sub,
                is_3d: false,
            } => unsafe {
                gl.framebuffer_texture_2d(
                    point,
                    attachment,
                    glow::TEXTURE_2D,
                    Some(raw),
                    sub.level_start as _,
                );
            },
            n::ImageView::Texture {
                target: _,
                raw,
                ref sub,
                is_3d: true,
            } => unsafe {
                gl.framebuffer_texture_layer(
                    point,
                    attachment,
                    Some(raw),
                    sub.level_start as _,
                    sub.layer_start as _,
                );
            },
        }
    }

    #[cfg(feature = "cross")]
    fn parse_spirv_cross(&self, raw_data: &[u32]) -> Result<CrossAst, d::ShaderError> {
        use spirv_cross::{spirv, ErrorCode as Ec};
        let module = spirv::Module::from_words(raw_data);

        spirv::Ast::parse(&module).map_err(|err| {
            d::ShaderError::CompilationFailed(match err {
                Ec::CompilationError(msg) => msg,
                Ec::Unhandled => "Unknown parsing error".into(),
            })
        })
    }

    #[cfg(feature = "cross")]
    fn set_push_const_layout(&self, _ast: &mut CrossAst) -> Result<(), d::ShaderError> {
        Ok(())
    }

    #[cfg(feature = "cross")]
    fn translate_spirv_cross(
        &self,
        ast: &mut CrossAst,
        stage: naga::ShaderStage,
        entry_point: &str,
    ) -> Result<String, d::ShaderError> {
        use spirv_cross::{glsl, ErrorCode as Ec};

        let mut compile_options = glsl::CompilerOptions::default();
        // see version table at https://en.wikipedia.org/wiki/OpenGL_Shading_Language
        let is_embedded = self.share.info.shading_language.is_embedded;
        let version = self.share.info.shading_language.tuple();
        compile_options.version = if is_embedded {
            match version {
                (3, 2) => glsl::Version::V3_20Es,
                (3, 1) => glsl::Version::V3_10Es,
                (3, 0) => glsl::Version::V3_00Es,
                (1, 0) => glsl::Version::V1_00Es,
                other if other > (3, 2) => glsl::Version::V3_20Es,
                other => panic!("GLSL version is not recognized: {:?}", other),
            }
        } else {
            match version {
                (4, 6) => glsl::Version::V4_60,
                (4, 5) => glsl::Version::V4_50,
                (4, 4) => glsl::Version::V4_40,
                (4, 3) => glsl::Version::V4_30,
                (4, 2) => glsl::Version::V4_20,
                (4, 1) => glsl::Version::V4_10,
                (4, 0) => glsl::Version::V4_00,
                (3, 3) => glsl::Version::V3_30,
                (1, 5) => glsl::Version::V1_50,
                (1, 4) => glsl::Version::V1_40,
                (1, 3) => glsl::Version::V1_30,
                (1, 2) => glsl::Version::V1_20,
                (1, 1) => glsl::Version::V1_10,
                other if other > (4, 6) => glsl::Version::V4_60,
                other => panic!("GLSL version is not recognized: {:?}", other),
            }
        };
        compile_options.vertex.invert_y = !self.features.contains(hal::Features::NDC_Y_UP);
        compile_options.force_zero_initialized_variables = true;
        compile_options.entry_point = Some((
            entry_point.to_string(),
            conv::map_naga_stage_to_cross(stage),
        ));
        log::debug!("SPIR-V options {:?}", compile_options);

        ast.set_compiler_options(&compile_options).map_err(|err| {
            d::ShaderError::CompilationFailed(match err {
                Ec::CompilationError(msg) => msg,
                Ec::Unhandled => "Unexpected error".into(),
            })
        })?;
        ast.compile().map_err(|err| {
            d::ShaderError::CompilationFailed(match err {
                Ec::CompilationError(msg) => msg,
                Ec::Unhandled => "Unknown compile error".into(),
            })
        })
    }

    #[cfg(feature = "cross")]
    fn remap_bindings(
        &self,
        ast: &mut CrossAst,
        layout: &n::PipelineLayout,
        nb_map: &mut FastHashMap<String, (n::BindingRegister, u8)>,
    ) {
        let res = ast.get_shader_resources().unwrap();
        self.remap_binding(
            ast,
            &res.sampled_images,
            n::BindingRegister::Textures,
            layout,
            nb_map,
        );
        self.remap_binding(
            ast,
            &res.uniform_buffers,
            n::BindingRegister::UniformBuffers,
            layout,
            nb_map,
        );
        self.remap_binding(
            ast,
            &res.storage_buffers,
            n::BindingRegister::StorageBuffers,
            layout,
            nb_map,
        );
    }

    #[cfg(feature = "cross")]
    fn remap_binding(
        &self,
        ast: &mut CrossAst,
        all_res: &[spirv_cross::spirv::Resource],
        register: n::BindingRegister,
        layout: &n::PipelineLayout,
        nb_map: &mut FastHashMap<String, (n::BindingRegister, u8)>,
    ) {
        use spirv_cross::spirv::Decoration;

        for res in all_res {
            let set = ast
                .get_decoration(res.id, Decoration::DescriptorSet)
                .unwrap();
            let binding = ast.get_decoration(res.id, Decoration::Binding).unwrap();
            let set_info = &layout.sets[set as usize];
            let slot = set_info.bindings[binding as usize];
            assert!((slot as usize) < MAX_TEXTURE_SLOTS);

            if self
                .share
                .legacy_features
                .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
            {
                ast.set_decoration(res.id, Decoration::Binding, slot as u32)
                    .unwrap()
            } else {
                ast.unset_decoration(res.id, Decoration::Binding).unwrap();
                assert!(nb_map.insert(res.name.clone(), (register, slot)).is_none());
            }
            ast.unset_decoration(res.id, Decoration::DescriptorSet)
                .unwrap();
        }
    }

    #[cfg(feature = "cross")]
    fn combine_separate_images_and_samplers(
        &self,
        ast: &mut CrossAst,
        context: CompilationContext,
    ) {
        use spirv_cross::spirv::Decoration;

        let mut id_map =
            FastHashMap::<u32, (pso::DescriptorSetIndex, pso::DescriptorBinding)>::default();
        let res = ast.get_shader_resources().unwrap();
        self.populate_id_map(ast, &mut id_map, &res.separate_images);
        self.populate_id_map(ast, &mut id_map, &res.separate_samplers);

        for cis in ast.get_combined_image_samplers().unwrap() {
            let texture_slot = {
                let &(set, binding) = id_map.get(&cis.image_id).unwrap();
                context.layout.sets[set as usize].bindings[binding as usize]
            };
            let sampler_slot = {
                let &(set, binding) = id_map.get(&cis.sampler_id).unwrap();
                context.layout.sets[set as usize].bindings[binding as usize]
            };
            context.sampler_map[texture_slot as usize] = Some(sampler_slot);

            if self
                .share
                .legacy_features
                .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
            {
                // if it was previously assigned, clear the binding of the image
                let _ = ast.unset_decoration(cis.image_id, Decoration::Binding);
                ast.set_decoration(cis.combined_id, Decoration::Binding, texture_slot as u32)
                    .unwrap()
            } else {
                let name = ast.get_name(cis.combined_id).unwrap();
                ast.unset_decoration(cis.combined_id, Decoration::Binding)
                    .unwrap();
                assert_eq!(
                    context
                        .name_binding_map
                        .insert(name, (n::BindingRegister::Textures, texture_slot)),
                    None
                );
            }
            ast.unset_decoration(cis.combined_id, Decoration::DescriptorSet)
                .unwrap();
        }
    }

    fn reflect_shader(
        module: &naga::Module,
        ep_info: &naga::valid::FunctionInfo,
        reflection_info: naga::back::glsl::ReflectionInfo,
        context: CompilationContext,
    ) {
        for (handle, var) in module.global_variables.iter() {
            if ep_info[handle].is_empty() {
                continue;
            }
            let register = match var.class {
                naga::StorageClass::Uniform => n::BindingRegister::UniformBuffers,
                naga::StorageClass::Storage => n::BindingRegister::StorageBuffers,
                _ => continue,
            };
            //TODO: make Naga reflect all the names, not just textures
            let slot = match var.binding {
                Some(ref br) => {
                    context.layout.sets[br.group as usize].bindings[br.binding as usize]
                }
                ref other => panic!("Unexpected resource binding {:?}", other),
            };

            let name = reflection_info.uniforms[&handle].clone();
            log::debug!("Rebind buffer: {:?} -> {}", var.name.as_ref(), &name);
            context.name_binding_map.insert(name, (register, slot));
        }

        for (name, mapping) in reflection_info.texture_mapping {
            let texture_linear_index = match module.global_variables[mapping.texture].binding {
                Some(ref br) => {
                    context.layout.sets[br.group as usize].bindings[br.binding as usize]
                }
                ref other => panic!("Unexpected texture binding {:?}", other),
            };
            context
                .name_binding_map
                .insert(name, (n::BindingRegister::Textures, texture_linear_index));
            if let Some(sampler_handle) = mapping.sampler {
                let sampler_linear_index = match module.global_variables[sampler_handle].binding {
                    Some(ref br) => {
                        context.layout.sets[br.group as usize].bindings[br.binding as usize]
                    }
                    ref other => panic!("Unexpected sampler binding {:?}", other),
                };
                context.sampler_map[texture_linear_index as usize] = Some(sampler_linear_index);
            }
        }
    }

    #[cfg(feature = "cross")]
    fn populate_id_map(
        &self,
        ast: &CrossAst,
        id_map: &mut FastHashMap<u32, (pso::DescriptorSetIndex, pso::DescriptorBinding)>,
        all_res: &[spirv_cross::spirv::Resource],
    ) {
        use spirv_cross::spirv::Decoration;
        for res in all_res {
            let set = ast
                .get_decoration(res.id, Decoration::DescriptorSet)
                .unwrap();
            let binding = ast.get_decoration(res.id, Decoration::Binding).unwrap();
            assert!(id_map.insert(res.id, (set as _, binding)).is_none())
        }
    }

    fn compile_shader_library_naga(
        gl: &GlContainer,
        shader: &d::NagaShader,
        options: &naga::back::glsl::Options,
        context: CompilationContext,
    ) -> Result<n::Shader, d::ShaderError> {
        let mut output = String::new();
        let mut writer =
            naga::back::glsl::Writer::new(&mut output, &shader.module, &shader.info, options)
                .map_err(|e| {
                    log::warn!("Naga GLSL init: {}", e);
                    d::ShaderError::CompilationFailed(format!("{:?}", e))
                })?;

        let entry_point_index = (&shader.module.entry_points)
            .into_iter()
            .position(|ep| ep.name == options.entry_point)
            .ok_or(d::ShaderError::CompilationFailed(format!(
                "Couldn't find entry point {}",
                options.entry_point
            )))?;

        match writer.write() {
            Ok(reflection_info) => {
                Self::reflect_shader(
                    &shader.module,
                    shader.info.get_entry_point(entry_point_index),
                    reflection_info,
                    context,
                );
                log::debug!("Naga generated shader:\n{}", output);
                Self::create_shader_module_raw(gl, &output, options.shader_stage)
            }
            Err(e) => {
                log::warn!("Naga GLSL write: {}", e);
                Err(d::ShaderError::CompilationFailed(format!("{:?}", e)))
            }
        }
    }

    fn compile_shader(
        &self,
        ep: &pso::EntryPoint<B>,
        stage: naga::ShaderStage,
        mut context: CompilationContext,
    ) -> Result<n::Shader, d::ShaderError> {
        let naga_options = naga::back::glsl::Options {
            version: {
                use naga::back::glsl::Version;
                let sl = &self.share.info.shading_language;
                let value = (sl.major * 100 + sl.minor * 10) as u16;
                if sl.is_embedded {
                    Version::Embedded(value)
                } else {
                    Version::Desktop(value)
                }
            },
            shader_stage: stage,
            entry_point: ep.entry.to_string(),
        };

        #[cfg_attr(not(feature = "cross"), allow(unused_mut))]
        let mut result = match ep.module.naga {
            Ok(ref shader) => Self::compile_shader_library_naga(
                &self.share.context,
                shader,
                &naga_options,
                context.reborrow(),
            ),
            Err(ref e) => Err(d::ShaderError::CompilationFailed(e.clone())),
        };
        #[cfg(feature = "cross")]
        if result.is_err() {
            let mut ast = self.parse_spirv_cross(&ep.module.spv).unwrap();
            auxil::spirv_cross_specialize_ast(&mut ast, &ep.specialization).unwrap();
            self.remap_bindings(&mut ast, context.layout, context.name_binding_map);
            self.combine_separate_images_and_samplers(&mut ast, context.reborrow());
            self.set_push_const_layout(&mut ast).unwrap();

            let glsl = self
                .translate_spirv_cross(&mut ast, stage, ep.entry)
                .unwrap();
            log::debug!("SPIRV-Cross generated shader:\n{}", glsl);
            result = Self::create_shader_module_raw(&self.share.context, &glsl, stage);
        }
        result
    }
}

pub(crate) unsafe fn set_sampler_info<SetParamFloat, SetParamFloatVec, SetParamInt>(
    info: &i::SamplerDesc,
    features: &hal::Features,
    mut set_param_float: SetParamFloat,
    mut set_param_float_vec: SetParamFloatVec,
    mut set_param_int: SetParamInt,
) where
    // TODO: Move these into a trait and implement for sampler/texture objects
    SetParamFloat: FnMut(u32, f32),
    SetParamFloatVec: FnMut(u32, &mut [f32]),
    SetParamInt: FnMut(u32, i32),
{
    let (min, mag) = conv::filter_to_gl(info.mag_filter, info.min_filter, info.mip_filter);
    if let Some(fac) = info.anisotropy_clamp {
        if features.contains(hal::Features::SAMPLER_ANISOTROPY) {
            set_param_float(glow::TEXTURE_MAX_ANISOTROPY, fac as f32);
        }
    }

    set_param_int(glow::TEXTURE_MIN_FILTER, min as i32);
    set_param_int(glow::TEXTURE_MAG_FILTER, mag as i32);

    let (s, t, r) = info.wrap_mode;
    set_param_int(glow::TEXTURE_WRAP_S, conv::wrap_to_gl(s) as i32);
    set_param_int(glow::TEXTURE_WRAP_T, conv::wrap_to_gl(t) as i32);
    set_param_int(glow::TEXTURE_WRAP_R, conv::wrap_to_gl(r) as i32);

    if features.contains(hal::Features::SAMPLER_MIP_LOD_BIAS) {
        set_param_float(glow::TEXTURE_LOD_BIAS, info.lod_bias.0);
    }
    if features.contains(hal::Features::SAMPLER_BORDER_COLOR) {
        let mut border: [f32; 4] = info.border.into();
        set_param_float_vec(glow::TEXTURE_BORDER_COLOR, &mut border);
    }

    set_param_float(glow::TEXTURE_MIN_LOD, info.lod_range.start.0);
    set_param_float(glow::TEXTURE_MAX_LOD, info.lod_range.end.0);

    match info.comparison {
        None => set_param_int(glow::TEXTURE_COMPARE_MODE, glow::NONE as i32),
        Some(cmp) => {
            set_param_int(
                glow::TEXTURE_COMPARE_MODE,
                glow::COMPARE_REF_TO_TEXTURE as i32,
            );
            set_param_int(
                glow::TEXTURE_COMPARE_FUNC,
                state::map_comparison(cmp) as i32,
            );
        }
    }
}

impl d::Device<B> for Device {
    unsafe fn allocate_memory(
        &self,
        mem_type: hal::MemoryTypeId,
        size: u64,
    ) -> Result<n::Memory, d::AllocationError> {
        let (memory_type, memory_role) = self.share.memory_types[mem_type.0 as usize];

        let is_device_local_memory = memory_type
            .properties
            .contains(memory::Properties::DEVICE_LOCAL);
        let is_cpu_visible_memory = memory_type
            .properties
            .contains(memory::Properties::CPU_VISIBLE);
        let is_coherent_memory = memory_type
            .properties
            .contains(memory::Properties::COHERENT);
        let is_readable_memory = memory_type
            .properties
            .contains(memory::Properties::CPU_CACHED);

        match memory_role {
            MemoryUsage::Buffer(buffer_usage) => {
                let gl = &self.share.context;
                let target = if buffer_usage.contains(buffer::Usage::INDEX)
                    && !self.share.private_caps.index_buffer_role_change
                {
                    glow::ELEMENT_ARRAY_BUFFER
                } else {
                    glow::ARRAY_BUFFER
                };

                let raw = gl.create_buffer().unwrap();
                //TODO: use *Named calls to avoid binding
                gl.bind_buffer(target, Some(raw));

                let mut map_flags = 0;

                if is_cpu_visible_memory {
                    map_flags |= glow::MAP_WRITE_BIT | glow::MAP_FLUSH_EXPLICIT_BIT;
                    if is_readable_memory {
                        map_flags |= glow::MAP_READ_BIT;
                    }
                }

                if self.share.private_caps.buffer_storage {
                    let mut storage_flags = 0;

                    if is_cpu_visible_memory {
                        map_flags |= glow::MAP_PERSISTENT_BIT;
                        storage_flags |= glow::MAP_WRITE_BIT
                            | glow::MAP_PERSISTENT_BIT
                            | glow::DYNAMIC_STORAGE_BIT;

                        if is_readable_memory {
                            storage_flags |= glow::MAP_READ_BIT;
                        }

                        if is_coherent_memory {
                            map_flags |= glow::MAP_COHERENT_BIT;
                            storage_flags |= glow::MAP_COHERENT_BIT;
                        }
                    }

                    gl.buffer_storage(target, size as i32, None, storage_flags);
                } else {
                    assert!(!is_coherent_memory);
                    let usage = if is_cpu_visible_memory {
                        if is_readable_memory {
                            glow::STREAM_READ
                        } else {
                            glow::DYNAMIC_DRAW
                        }
                    } else {
                        glow::STATIC_DRAW
                    };
                    gl.buffer_data_size(target, size as i32, usage);
                }

                gl.bind_buffer(target, None);

                if let Err(err) = self.share.check() {
                    panic!("Error allocating memory buffer {:?}", err);
                }

                Ok(n::Memory {
                    properties: memory_type.properties,
                    buffer: Some((raw, target)),
                    size,
                    map_flags,
                    emulate_map_allocation: None,
                })
            }

            MemoryUsage::Image => {
                assert!(is_device_local_memory);
                Ok(n::Memory {
                    properties: memory::Properties::DEVICE_LOCAL,
                    buffer: None,
                    size,
                    map_flags: 0,
                    emulate_map_allocation: None,
                })
            }
        }
    }

    unsafe fn create_command_pool(
        &self,
        _family: queue::QueueFamilyId,
        flags: CommandPoolCreateFlags,
    ) -> Result<CommandPool, d::OutOfMemory> {
        let fbo = create_fbo_internal(&self.share);
        let limits = self.share.public_caps.limits.into();
        let memory = if flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
            BufferMemory::Individual {
                storage: FastHashMap::default(),
                next_buffer_id: 0,
            }
        } else {
            BufferMemory::Linear(OwnedBuffer::new())
        };

        // Ignoring `TRANSIENT` hint, unsure how to make use of this.

        Ok(CommandPool {
            fbo,
            limits,
            memory: Arc::new(Mutex::new(memory)),
            legacy_features: self.share.legacy_features,
        })
    }

    unsafe fn destroy_command_pool(&self, pool: CommandPool) {
        if let Some(fbo) = pool.fbo {
            let gl = &self.share.context;
            gl.delete_framebuffer(fbo);
        }
    }

    unsafe fn create_render_pass<'a, Ia, Is, Id>(
        &self,
        attachments: Ia,
        subpasses: Is,
        _dependencies: Id,
    ) -> Result<n::RenderPass, d::OutOfMemory>
    where
        Ia: Iterator<Item = pass::Attachment>,
        Is: Iterator<Item = pass::SubpassDesc<'a>>,
    {
        let subpasses = subpasses
            .map(|subpass| {
                assert!(
                    subpass.colors.len() <= self.share.public_caps.limits.max_color_attachments,
                    "Color attachment limit exceeded"
                );
                let color_attachments = subpass.colors.iter().map(|&(index, _)| index).collect();

                let depth_stencil = subpass.depth_stencil.map(|ds| ds.0);

                n::SubpassDesc {
                    color_attachments,
                    depth_stencil,
                }
            })
            .collect();

        Ok(n::RenderPass {
            attachments: attachments.collect::<Vec<_>>(),
            subpasses,
        })
    }

    unsafe fn create_pipeline_layout<'a, Is, Ic>(
        &self,
        layouts: Is,
        _: Ic,
    ) -> Result<n::PipelineLayout, d::OutOfMemory>
    where
        Is: Iterator<Item = &'a n::DescriptorSetLayout>,
    {
        use std::convert::TryInto;
        let mut sets = Vec::new();
        let mut num_samplers = 0usize;
        let mut num_textures = 0usize;
        let mut num_uniform_buffers = 0usize;
        let mut num_storage_buffers = 0usize;

        for layout_bindings in layouts {
            // create a vector with the size enough to hold all the bindings, filled with `!0`
            let mut bindings =
                vec![!0; layout_bindings.last().map_or(0, |b| b.binding as usize + 1)];

            for binding in layout_bindings.iter() {
                assert!(!binding.immutable_samplers); //TODO
                let counter = match binding.ty {
                    pso::DescriptorType::Sampler => &mut num_samplers,
                    pso::DescriptorType::InputAttachment | pso::DescriptorType::Image { .. } => {
                        &mut num_textures
                    }
                    pso::DescriptorType::Buffer {
                        ty,
                        format: _, //TODO
                    } => match ty {
                        pso::BufferDescriptorType::Uniform => &mut num_uniform_buffers,
                        pso::BufferDescriptorType::Storage { .. } => &mut num_storage_buffers,
                    },
                };

                bindings[binding.binding as usize] = (*counter).try_into().unwrap();
                *counter += binding.count;
            }

            sets.push(n::PipelineLayoutSet {
                layout: Arc::clone(layout_bindings),
                bindings,
            });
        }

        Ok(n::PipelineLayout { sets })
    }

    unsafe fn create_pipeline_cache(&self, _data: Option<&[u8]>) -> Result<(), d::OutOfMemory> {
        Ok(())
    }

    unsafe fn get_pipeline_cache_data(&self, _cache: &()) -> Result<Vec<u8>, d::OutOfMemory> {
        //empty
        Ok(Vec::new())
    }

    unsafe fn destroy_pipeline_cache(&self, _: ()) {
        //empty
    }

    unsafe fn merge_pipeline_caches<'a, I>(&self, _: &mut (), _: I) -> Result<(), d::OutOfMemory>
    where
        I: Iterator<Item = &'a ()>,
    {
        //empty
        Ok(())
    }

    unsafe fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
        _cache: Option<&()>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let (vertex_buffers, desc_attributes, input_assembler, vs) = match desc.primitive_assembler
        {
            pso::PrimitiveAssemblerDesc::Vertex {
                buffers,
                attributes,
                ref input_assembler,
                ref vertex,
                ref tessellation,
                ref geometry,
            } => {
                if tessellation.is_some() || geometry.is_some() {
                    return Err(pso::CreationError::UnsupportedPipeline);
                }

                let mut vertex_buffers = Vec::new();
                for vb in buffers {
                    while vertex_buffers.len() <= vb.binding as usize {
                        vertex_buffers.push(None);
                    }
                    vertex_buffers[vb.binding as usize] = Some(*vb);
                }

                (vertex_buffers, attributes, input_assembler, vertex)
            }
            pso::PrimitiveAssemblerDesc::Mesh { .. } => {
                return Err(pso::CreationError::UnsupportedPipeline);
            }
        };

        let shaders = [
            (naga::ShaderStage::Vertex, Some(vs)),
            (naga::ShaderStage::Fragment, desc.fragment.as_ref()),
        ];
        let (program, sampler_map) = self.create_shader_program(&shaders[..], &desc.layout)?;

        let patch_size = match input_assembler.primitive {
            pso::Primitive::PatchList(size) => Some(size as _),
            _ => None,
        };

        let mut uniforms = Vec::new();
        {
            let gl = &self.share.context;
            let count = gl.get_active_uniforms(program);

            let mut offset = 0;

            for uniform in 0..count {
                let glow::ActiveUniform { size, utype, name } =
                    gl.get_active_uniform(program, uniform).unwrap();

                if let Some(location) = gl.get_uniform_location(program, &name) {
                    // Sampler2D won't show up in UniformLocation and the only other uniforms
                    // should be push constants
                    uniforms.push(n::UniformDesc {
                        location: Starc::new(location),
                        offset,
                        utype,
                    });

                    offset += size as u32;
                }
            }
        }

        Ok(n::GraphicsPipeline {
            program,
            primitive: conv::input_assember_to_gl_primitive(input_assembler),
            patch_size,
            blend_targets: desc.blender.targets.clone(),
            vertex_buffers,
            attributes: desc_attributes
                .iter()
                .map(|&a| {
                    let fd = conv::describe_format(a.element.format).unwrap();
                    n::AttributeDesc {
                        location: a.location,
                        offset: a.element.offset,
                        binding: a.binding,
                        size: fd.num_components as _,
                        format: fd.data_type,
                        vertex_attrib_fn: fd.va_fun,
                    }
                })
                .collect(),
            uniforms,
            rasterizer: desc.rasterizer,
            depth: desc.depth_stencil.depth,
            baked_states: desc.baked_states.clone(),
            sampler_map,
        })
    }

    unsafe fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
        _cache: Option<&()>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        if self.share.public_caps.limits.max_compute_work_group_count[0] == 0 {
            return Err(pso::CreationError::UnsupportedPipeline);
        }
        let shader = (naga::ShaderStage::Compute, Some(&desc.shader));
        let (program, sampler_map) = self.create_shader_program(&[shader], &desc.layout)?;
        Ok(n::ComputePipeline {
            program,
            sampler_map,
        })
    }

    unsafe fn create_framebuffer<I>(
        &self,
        _render_pass: &n::RenderPass,
        _attachments: I,
        _extent: i::Extent,
    ) -> Result<n::Framebuffer, d::OutOfMemory> {
        if !self.share.private_caps.framebuffer {
            return Err(d::OutOfMemory::Host);
        }

        let gl = &self.share.context;
        let raw = gl.create_framebuffer().unwrap();

        /*
        let attachments: Vec<_> = attachments

            .map(|at| at.borrow().clone())
            .collect();
        log::debug!("create_framebuffer {:?}", attachments);

        let target = glow::DRAW_FRAMEBUFFER;

        let fbos = pass.subpasses.iter().map(|subpass| {
            let name = gl.create_framebuffer().unwrap();
            gl.bind_framebuffer(target, Some(name));

            for (index, &color) in subpass.color_attachments.iter().enumerate() {
                let color_attachment = glow::COLOR_ATTACHMENT0 + index as u32;
                assert!(color_attachment <= glow::COLOR_ATTACHMENT31);

                if self.share.private_caps.framebuffer_texture {
                    Self::bind_target(gl, target, color_attachment, &attachments[color]);
                } else {
                    Self::bind_target_compat(gl, target, color_attachment, &attachments[color]);
                }
            }

            if let Some(depth_stencil) = subpass.depth_stencil {
                let aspects = match attachments[depth_stencil] {
                    n::ImageView::Texture { ref sub, .. } => sub.aspects,
                    n::ImageView::Renderbuffer { aspects, .. } => aspects,
                };

                let attachment = if aspects == Aspects::DEPTH {
                    glow::DEPTH_ATTACHMENT
                } else if aspects == Aspects::STENCIL {
                    glow::STENCIL_ATTACHMENT
                } else {
                    glow::DEPTH_STENCIL_ATTACHMENT
                };

                if self.share.private_caps.framebuffer_texture {
                    Self::bind_target(gl, target, attachment, &attachments[depth_stencil]);
                } else {
                    Self::bind_target_compat(gl, target, attachment, &attachments[depth_stencil]);
                }
            }

            let status = gl.check_framebuffer_status(target);
            match status {
                glow::FRAMEBUFFER_COMPLETE => {},
                glow::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => panic!("One of framebuffer attachmet points are incomplete"),
                glow::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => panic!("Framebuffer does not have any image attached"),
                glow::FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER => panic!("FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER"),
                glow::FRAMEBUFFER_INCOMPLETE_READ_BUFFER => panic!("FRAMEBUFFER_INCOMPLETE_READ_BUFFER"),
                glow::FRAMEBUFFER_UNSUPPORTED => panic!("FRAMEBUFFER_UNSUPPORTED"),
                glow::FRAMEBUFFER_INCOMPLETE_MULTISAMPLE => panic!("FRAMEBUFFER_INCOMPLETE_MULTISAMPLE"),
                glow::FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS => panic!("FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS"),
                36057 /*glow::FRAMEBUFFER_INCOMPLETE_DIMENSIONS*/ => panic!("Framebuffer attachements have different dimensions"),
                code => panic!("Unexpected framebuffer status code {}", code),
            }

            if let Err(err) = self.share.check() {
                //TODO: attachments have been consumed
                panic!("Error creating FBO: {:?} for {:?}", err, pass);
            }

            Some(name)
        }).collect();

        gl.bind_framebuffer(target, None);*/

        Ok(n::Framebuffer { raw })
    }

    unsafe fn create_shader_module(
        &self,
        raw_data: &[u32],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        Ok(n::ShaderModule {
            #[cfg(feature = "cross")]
            spv: raw_data.to_vec(),
            naga: if cfg!(feature = "cross") {
                Err("Cross is enabled".into())
            } else {
                let options = naga::front::spv::Options {
                    adjust_coordinate_space: !self.features.contains(hal::Features::NDC_Y_UP),
                    strict_capabilities: true,
                    flow_graph_dump_prefix: None,
                };
                let parser = naga::front::spv::Parser::new(raw_data.iter().cloned(), &options);
                match parser.parse() {
                    Ok(module) => {
                        log::debug!("Naga module {:#?}", module);
                        match naga::valid::Validator::new(
                            naga::valid::ValidationFlags::empty(),
                            naga::valid::Capabilities::empty(), //TODO: PUSH_CONSTANT
                        )
                        .validate(&module)
                        {
                            Ok(info) => Ok(d::NagaShader { module, info }),
                            Err(e) => Err(format!("Naga validation: {}", e)),
                        }
                    }
                    Err(e) => Err(format!("Naga parsing: {:?}", e)),
                }
            },
        })
    }

    unsafe fn create_shader_module_from_naga(
        &self,
        shader: d::NagaShader,
    ) -> Result<n::ShaderModule, (d::ShaderError, d::NagaShader)> {
        Ok(n::ShaderModule {
            #[cfg(feature = "cross")]
            spv: match naga::back::spv::write_vec(&shader.module, &shader.info, &self.spv_options) {
                Ok(spv) => spv,
                Err(e) => {
                    return Err((d::ShaderError::CompilationFailed(format!("{}", e)), shader))
                }
            },
            naga: Ok(shader),
        })
    }

    unsafe fn create_sampler(
        &self,
        info: &i::SamplerDesc,
    ) -> Result<n::FatSampler, d::AllocationError> {
        assert!(info.normalized);

        if !self
            .share
            .legacy_features
            .contains(LegacyFeatures::SAMPLER_OBJECTS)
        {
            return Ok(n::FatSampler::Info(info.clone()));
        }

        let gl = &self.share.context;

        let name = gl.create_sampler().unwrap();
        set_sampler_info(
            &info,
            &self.features,
            |a, b| gl.sampler_parameter_f32(name, a, b),
            |a, b| gl.sampler_parameter_f32_slice(name, a, b),
            |a, b| gl.sampler_parameter_i32(name, a, b),
        );

        if let Err(_) = self.share.check() {
            Err(d::AllocationError::OutOfMemory(d::OutOfMemory::Host))
        } else {
            Ok(n::FatSampler::Sampler(name))
        }
    }

    unsafe fn create_buffer(
        &self,
        size: u64,
        usage: buffer::Usage,
        _sparse: memory::SparseFlags,
    ) -> Result<n::Buffer, buffer::CreationError> {
        if !self
            .share
            .legacy_features
            .contains(LegacyFeatures::CONSTANT_BUFFER)
            && usage.contains(buffer::Usage::UNIFORM)
        {
            return Err(buffer::CreationError::UnsupportedUsage(usage));
        }

        Ok(n::Buffer::Unbound { size, usage })
    }

    unsafe fn get_buffer_requirements(&self, buffer: &n::Buffer) -> memory::Requirements {
        let (size, usage) = match *buffer {
            n::Buffer::Unbound { size, usage } => (size, usage),
            n::Buffer::Bound { .. } => panic!("Unexpected Buffer::Bound"),
        };

        memory::Requirements {
            size: size as u64,
            // Alignment of 4 covers indexes of type u16 and u32 in index buffers, which is
            // currently the only alignment requirement.
            alignment: 4,
            type_mask: self.share.buffer_memory_type_mask(usage),
        }
    }

    unsafe fn bind_buffer_memory(
        &self,
        memory: &n::Memory,
        offset: u64,
        buffer: &mut n::Buffer,
    ) -> Result<(), d::BindError> {
        let size = match *buffer {
            n::Buffer::Unbound { size, .. } => size,
            n::Buffer::Bound { .. } => panic!("Unexpected Buffer::Bound"),
        };

        match memory.buffer {
            Some((raw, target)) => {
                *buffer = n::Buffer::Bound {
                    buffer: raw,
                    range: offset..offset + size,
                    target: target,
                };
            }
            None => {
                panic!("Improper memory type used for buffer memory");
            }
        }

        Ok(())
    }

    unsafe fn map_memory(
        &self,
        memory: &mut n::Memory,
        segment: memory::Segment,
    ) -> Result<*mut u8, d::MapError> {
        let gl = &self.share.context;
        let caps = &self.share.private_caps;

        let offset = segment.offset;
        let size = segment.size.unwrap_or(memory.size - segment.offset);

        let (buffer, target) = memory.buffer.expect("cannot map image memory");
        let ptr = if caps.emulate_map {
            let ptr: *mut u8 = if let Some(ptr) = memory.emulate_map_allocation {
                ptr
            } else {
                let ptr =
                    Box::into_raw(vec![0; memory.size as usize].into_boxed_slice()) as *mut u8;
                memory.emulate_map_allocation = Some(ptr);
                ptr
            };

            ptr.offset(offset as isize)
        } else {
            gl.bind_buffer(target, Some(buffer));
            let raw = gl.map_buffer_range(target, offset as i32, size as i32, memory.map_flags);
            gl.bind_buffer(target, None);
            raw
        };

        if let Err(err) = self.share.check() {
            panic!("Error mapping memory: {:?} for memory {:?}", err, memory);
        }

        Ok(ptr)
    }

    unsafe fn unmap_memory(&self, memory: &mut n::Memory) {
        let gl = &self.share.context;
        let (buffer, target) = memory.buffer.expect("cannot unmap image memory");

        gl.bind_buffer(target, Some(buffer));

        if self.share.private_caps.emulate_map {
            let ptr = memory.emulate_map_allocation.take().unwrap();
            let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, memory.size as usize));
        } else {
            gl.unmap_buffer(target);
        }

        gl.bind_buffer(target, None);

        if let Err(err) = self.share.check() {
            panic!("Error unmapping memory: {:?} for memory {:?}", err, memory);
        }
    }

    unsafe fn flush_mapped_memory_ranges<'a, I>(&self, ranges: I) -> Result<(), d::OutOfMemory>
    where
        I: Iterator<Item = (&'a n::Memory, memory::Segment)>,
    {
        let gl = &self.share.context;

        for (mem, segment) in ranges {
            let (buffer, target) = mem.buffer.expect("cannot flush image memory");
            gl.bind_buffer(target, Some(buffer));

            let offset = segment.offset;
            let size = segment.size.unwrap_or(mem.size - segment.offset);

            if self.share.private_caps.emulate_map {
                let ptr = mem.emulate_map_allocation.unwrap();
                let slice = slice::from_raw_parts_mut(ptr.offset(offset as isize), size as usize);
                gl.buffer_sub_data_u8_slice(target, offset as i32, slice);
            } else {
                gl.flush_mapped_buffer_range(target, offset as i32, size as i32);
            }
            gl.bind_buffer(target, None);
            if let Err(err) = self.share.check() {
                panic!(
                    "Error flushing memory range: {:?} for memory {:?}",
                    err, mem
                );
            }
        }

        Ok(())
    }

    unsafe fn invalidate_mapped_memory_ranges<'a, I>(&self, ranges: I) -> Result<(), d::OutOfMemory>
    where
        I: Iterator<Item = (&'a n::Memory, memory::Segment)>,
    {
        let gl = &self.share.context;

        for (mem, segment) in ranges {
            let (buffer, target) = mem.buffer.expect("cannot invalidate image memory");
            gl.bind_buffer(target, Some(buffer));

            let offset = segment.offset;
            let size = segment.size.unwrap_or(mem.size - segment.offset);

            if self.share.private_caps.emulate_map {
                let ptr = mem.emulate_map_allocation.unwrap();
                let slice = slice::from_raw_parts_mut(ptr.offset(offset as isize), size as usize);
                gl.get_buffer_sub_data(target, offset as i32, slice);
            } else {
                gl.invalidate_buffer_sub_data(target, offset as i32, size as i32);
                gl.bind_buffer(target, None);
            }

            if let Err(err) = self.share.check() {
                panic!(
                    "Error invalidating memory range: {:?} for memory {:?}",
                    err, mem
                );
            }
        }

        Ok(())
    }

    unsafe fn create_buffer_view(
        &self,
        _: &n::Buffer,
        _: Option<Format>,
        _: buffer::SubRange,
    ) -> Result<n::BufferView, buffer::ViewCreationError> {
        unimplemented!()
    }

    unsafe fn create_image(
        &self,
        kind: i::Kind,
        num_levels: i::Level,
        format: Format,
        _tiling: i::Tiling,
        usage: i::Usage,
        _sparse: memory::SparseFlags,
        _view_caps: i::ViewCapabilities,
    ) -> Result<n::Image, i::CreationError> {
        let gl = &self.share.context;

        let desc = conv::describe_format(format).ok_or(i::CreationError::Format(format))?;
        let channel = format.base_format().1;

        let mut pixel_count: u64 = 0;
        let image = if num_levels > 1 || usage.intersects(i::Usage::STORAGE | i::Usage::SAMPLED) {
            let name = gl.create_texture().unwrap();
            let target = match kind {
                i::Kind::D2(w, h, 1, 1) => {
                    gl.bind_texture(glow::TEXTURE_2D, Some(name));
                    if self.share.private_caps.image_storage {
                        gl.tex_storage_2d(
                            glow::TEXTURE_2D,
                            num_levels as _,
                            desc.tex_internal,
                            w as _,
                            h as _,
                        );
                        pixel_count += (w * h) as u64 * num_levels as u64;
                    } else {
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_MAX_LEVEL,
                            (num_levels - 1) as _,
                        );
                        let mut w = w;
                        let mut h = h;
                        for i in 0..num_levels {
                            gl.tex_image_2d(
                                glow::TEXTURE_2D,
                                i as _,
                                desc.tex_internal as i32,
                                w as _,
                                h as _,
                                0,
                                desc.tex_external,
                                desc.data_type,
                                None,
                            );
                            pixel_count += (w * h) as u64;
                            w = std::cmp::max(w / 2, 1);
                            h = std::cmp::max(h / 2, 1);
                        }
                    }
                    match channel {
                        ChannelType::Uint | ChannelType::Sint => {
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_MIN_FILTER,
                                glow::NEAREST as _,
                            );
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_MAG_FILTER,
                                glow::NEAREST as _,
                            );
                        }
                        _ => {}
                    };
                    glow::TEXTURE_2D
                }
                i::Kind::D2(w, h, l, 1) => {
                    gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(name));
                    if self.share.private_caps.image_storage {
                        gl.tex_storage_3d(
                            glow::TEXTURE_2D_ARRAY,
                            num_levels as _,
                            desc.tex_internal,
                            w as _,
                            h as _,
                            l as _,
                        );
                        pixel_count += (w * h) as u64 * l as u64 * num_levels as u64;
                    } else {
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D_ARRAY,
                            glow::TEXTURE_MAX_LEVEL,
                            (num_levels - 1) as _,
                        );
                        let mut w = w;
                        let mut h = h;
                        for i in 0..num_levels {
                            gl.tex_image_3d(
                                glow::TEXTURE_2D_ARRAY,
                                i as _,
                                desc.tex_internal as i32,
                                w as _,
                                h as _,
                                l as _,
                                0,
                                desc.tex_external,
                                desc.data_type,
                                None,
                            );
                            pixel_count += (w * h) as u64 * l as u64;
                            w = std::cmp::max(w / 2, 1);
                            h = std::cmp::max(h / 2, 1);
                        }
                    }
                    match channel {
                        ChannelType::Uint | ChannelType::Sint => {
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_MIN_FILTER,
                                glow::NEAREST as _,
                            );
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_MAG_FILTER,
                                glow::NEAREST as _,
                            );
                        }
                        _ => {}
                    };
                    glow::TEXTURE_2D_ARRAY
                }
                _ => unimplemented!(),
            };
            n::ImageType::Texture {
                target,
                raw: name,
                format: desc.tex_external,
                pixel_type: desc.data_type,
                layer_count: kind.num_layers(),
                level_count: num_levels,
            }
        } else {
            let name = gl.create_renderbuffer().unwrap();
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(name));
            match kind {
                i::Kind::D2(w, h, 1, 1) => {
                    gl.renderbuffer_storage(glow::RENDERBUFFER, desc.tex_internal, w as _, h as _);
                    pixel_count += (w * h) as u64;
                }
                i::Kind::D2(w, h, 1, samples) => {
                    gl.renderbuffer_storage_multisample(
                        glow::RENDERBUFFER,
                        samples as _,
                        desc.tex_internal,
                        w as _,
                        h as _,
                    );
                    pixel_count += (w * h) as u64 * samples as u64; // Not sure though
                }
                _ => unimplemented!(),
            };
            n::ImageType::Renderbuffer {
                raw: name,
                format: desc.tex_external,
            }
        };

        let surface_desc = format.base_format().0.desc();
        let bytes_per_texel = surface_desc.bits / 8;
        let size = pixel_count as u64 * bytes_per_texel as u64;
        let type_mask = self.share.image_memory_type_mask();

        if let Err(err) = self.share.check() {
            panic!(
                "Error creating image: {:?} for kind {:?} of {:?}",
                err, kind, format
            );
        }

        Ok(n::Image {
            object_type: image,
            kind,
            format_desc: surface_desc,
            channel,
            requirements: memory::Requirements {
                size,
                alignment: 1,
                type_mask,
            },
            num_levels,
            num_layers: kind.num_layers(),
        })
    }

    unsafe fn get_image_requirements(&self, unbound: &n::Image) -> memory::Requirements {
        unbound.requirements
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        image: &n::Image,
        sub: i::Subresource,
    ) -> i::SubresourceFootprint {
        let num_layers = image.kind.num_layers() as buffer::Offset;
        let level_offset = (0..sub.level).fold(0, |offset, level| {
            let pitches = image.pitches(level);
            offset + num_layers * pitches[3]
        });
        let pitches = image.pitches(sub.level);
        let layer_offset = level_offset + sub.layer as buffer::Offset * pitches[3];
        i::SubresourceFootprint {
            slice: layer_offset..layer_offset + pitches[3],
            row_pitch: pitches[1] as _,
            depth_pitch: pitches[2] as _,
            array_pitch: pitches[3] as _,
        }
    }

    unsafe fn bind_image_memory(
        &self,
        _memory: &n::Memory,
        _offset: u64,
        _image: &mut n::Image,
    ) -> Result<(), d::BindError> {
        Ok(())
    }

    unsafe fn create_image_view(
        &self,
        image: &n::Image,
        kind: i::ViewKind,
        view_format: Format,
        swizzle: Swizzle,
        _usage: i::Usage,
        range: i::SubresourceRange,
    ) -> Result<n::ImageView, i::ViewCreationError> {
        assert_eq!(swizzle, Swizzle::NO);
        match image.object_type {
            n::ImageType::Renderbuffer { raw, .. } => {
                let level = range.level_start;
                if range.level_start == 0 && range.layer_start == 0 {
                    Ok(n::ImageView::Renderbuffer {
                        raw,
                        aspects: image.format_desc.aspects,
                    })
                } else if level != 0 {
                    Err(i::ViewCreationError::Level(level)) //TODO
                } else {
                    Err(i::ViewCreationError::Layer(i::LayerError::OutOfBounds))
                }
            }
            n::ImageType::Texture {
                target,
                raw,
                format,
                ..
            } => {
                let is_3d = match kind {
                    i::ViewKind::D1 | i::ViewKind::D2 => false,
                    _ => true,
                };
                match conv::describe_format(view_format) {
                    Some(description) => {
                        let raw_view_format = description.tex_external;
                        if format != raw_view_format {
                            log::warn!(
                                "View format {:?} is different from base {:?}",
                                raw_view_format,
                                format
                            );
                        }
                    }
                    None => {
                        log::warn!("View format {:?} is not supported", view_format);
                    }
                }
                Ok(n::ImageView::Texture {
                    target,
                    raw,
                    is_3d,
                    sub: range,
                })
            }
        }
    }

    unsafe fn create_descriptor_pool<I>(
        &self,
        _: usize,
        _: I,
        _: pso::DescriptorPoolCreateFlags,
    ) -> Result<n::DescriptorPool, d::OutOfMemory>
    where
        I: Iterator<Item = pso::DescriptorRangeDesc>,
    {
        Ok(n::DescriptorPool {})
    }

    unsafe fn create_descriptor_set_layout<'a, I, J>(
        &self,
        layout: I,
        _immutable_samplers: J,
    ) -> Result<n::DescriptorSetLayout, d::OutOfMemory>
    where
        I: Iterator<Item = pso::DescriptorSetLayoutBinding>,
        J: Iterator<Item = &'a n::FatSampler>,
    {
        let mut bindings = layout.collect::<Vec<_>>();
        // all operations rely on the ascending bindings order
        bindings.sort_by_key(|b| b.binding);
        Ok(Arc::new(bindings))
    }

    unsafe fn write_descriptor_set<'a, I>(&self, op: pso::DescriptorSetWrite<'a, B, I>)
    where
        I: Iterator<Item = pso::Descriptor<'a, B>>,
    {
        let mut layout_index = op
            .set
            .layout
            .binary_search_by_key(&op.binding, |b| b.binding)
            .unwrap();
        let mut array_offset = op.array_offset;

        for descriptor in op.descriptors {
            let binding_layout = &op.set.layout[layout_index];
            let binding = match descriptor {
                pso::Descriptor::Buffer(buffer, ref sub) => {
                    let bounded_buffer = buffer.as_bound();
                    let range = crate::resolve_sub_range(sub, bounded_buffer.range);

                    let register = match binding_layout.ty {
                        pso::DescriptorType::Buffer { ty, .. } => match ty {
                            pso::BufferDescriptorType::Uniform => {
                                n::BindingRegister::UniformBuffers
                            }
                            pso::BufferDescriptorType::Storage { .. } => {
                                n::BindingRegister::StorageBuffers
                            }
                        },
                        other => {
                            panic!("Can't write buffer into descriptor of type {:?}", other)
                        }
                    };

                    n::DescSetBindings::Buffer {
                        register,
                        buffer: bounded_buffer.raw,
                        offset: range.start as i32,
                        size: (range.end - range.start) as i32,
                    }
                }
                pso::Descriptor::CombinedImageSampler(view, _layout, sampler) => {
                    match *view {
                        n::ImageView::Texture { target, raw, .. } => op
                            .set
                            .bindings
                            .push(n::DescSetBindings::Texture(raw, target)),
                        n::ImageView::Renderbuffer { .. } => {
                            panic!("Texture doesn't support shader binding")
                        }
                    }
                    match *sampler {
                        n::FatSampler::Sampler(sampler) => n::DescSetBindings::Sampler(sampler),
                        n::FatSampler::Info(ref info) => {
                            n::DescSetBindings::SamplerDesc(info.clone())
                        }
                    }
                }
                pso::Descriptor::Image(view, _layout) => match *view {
                    n::ImageView::Texture { target, raw, .. } => {
                        n::DescSetBindings::Texture(raw, target)
                    }
                    n::ImageView::Renderbuffer { .. } => {
                        panic!("Texture doesn't support shader binding")
                    }
                },
                pso::Descriptor::Sampler(sampler) => match *sampler {
                    n::FatSampler::Sampler(sampler) => n::DescSetBindings::Sampler(sampler),
                    n::FatSampler::Info(ref info) => n::DescSetBindings::SamplerDesc(info.clone()),
                },
                pso::Descriptor::TexelBuffer(_view) => unimplemented!(),
            };

            //TODO: overwrite instead of pushing on top
            op.set.bindings.push(binding);

            array_offset += 1;
            if array_offset == binding_layout.count {
                array_offset = 0;
                layout_index += 1;
            }
        }
    }

    unsafe fn copy_descriptor_set<'a>(&self, op: pso::DescriptorSetCopy<'a, B>) {
        if std::ptr::eq(op.src_set, &*op.dst_set) {
            panic!("copying within same descriptor set is not currently supported");
        }

        // TODO: add support for array bindings when the OpenGL backend gets them
        let src_start = op.src_binding as usize;
        let src_end = src_start + op.count;
        assert!(src_end <= op.src_set.bindings.len());

        let src_slice = &op.src_set.bindings[src_start..src_end];

        let dst_start = op.dst_binding as usize;
        let dst_end = dst_start + op.count;
        assert!(dst_end <= op.dst_set.bindings.len());

        op.dst_set.bindings[dst_start..dst_end].clone_from_slice(src_slice);
    }

    fn create_semaphore(&self) -> Result<n::Semaphore, d::OutOfMemory> {
        Ok(n::Semaphore)
    }

    fn create_fence(&self, signaled: bool) -> Result<n::Fence, d::OutOfMemory> {
        Ok(n::Fence::Idle { signaled })
    }

    unsafe fn reset_fence(&self, fence: &mut n::Fence) -> Result<(), d::OutOfMemory> {
        *fence = n::Fence::Idle { signaled: false };
        Ok(())
    }

    unsafe fn wait_for_fence(
        &self,
        fence: &n::Fence,
        timeout_ns: u64,
    ) -> Result<bool, d::WaitError> {
        // TODO:
        // This can be called by multiple objects wanting to ensure they have exclusive
        // access to a resource. How much does this call costs ? The status of the fence
        // could be cached to avoid calling this more than once (in core or in the backend ?).
        let gl = &self.share.context;
        match *fence {
            n::Fence::Idle { signaled } => {
                if !signaled {
                    log::warn!("Fence ptr {:?} is not pending, waiting not possible", fence);
                }
                Ok(signaled)
            }
            n::Fence::Pending(sync) => {
                // TODO: Could `wait_sync` be used here instead?
                match gl.client_wait_sync(sync, glow::SYNC_FLUSH_COMMANDS_BIT, timeout_ns as i32) {
                    glow::TIMEOUT_EXPIRED => Ok(false),
                    glow::WAIT_FAILED => {
                        if let Err(err) = self.share.check() {
                            log::error!("Error when waiting on fence: {:?}", err);
                        }
                        Ok(false)
                    }
                    glow::CONDITION_SATISFIED | glow::ALREADY_SIGNALED => {
                        //fence.0.set(n::Fence::Idle { signaled: true });
                        Ok(true)
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    unsafe fn wait_for_fences<'a, I>(
        &self,
        fences: I,
        wait: d::WaitFor,
        timeout_ns: u64,
    ) -> Result<bool, d::WaitError>
    where
        I: Iterator<Item = &'a n::Fence>,
    {
        let performance = web_sys::window().unwrap().performance().unwrap();
        let start = performance.now();
        let get_elapsed = || ((performance.now() - start) * 1_000_000.0) as u64;

        match wait {
            d::WaitFor::All => {
                for fence in fences {
                    if !self.wait_for_fence(fence, 0)? {
                        let elapsed_ns = get_elapsed();
                        if elapsed_ns > timeout_ns {
                            return Ok(false);
                        }
                        if !self.wait_for_fence(fence, timeout_ns - elapsed_ns)? {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
            d::WaitFor::Any => {
                const FENCE_WAIT_NS: u64 = 100_000;

                let fences: Vec<_> = fences.collect();
                loop {
                    for fence in &fences {
                        if self.wait_for_fence(fence, FENCE_WAIT_NS)? {
                            return Ok(true);
                        }
                    }
                    if get_elapsed() >= timeout_ns {
                        return Ok(false);
                    }
                }
            }
        }
    }

    unsafe fn get_fence_status(&self, fence: &n::Fence) -> Result<bool, d::DeviceLost> {
        Ok(match *fence {
            n::Fence::Idle { signaled } => signaled,
            n::Fence::Pending(sync) => self.share.context.get_sync_status(sync) == glow::SIGNALED,
        })
    }

    fn create_event(&self) -> Result<(), d::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn get_event_status(&self, _event: &()) -> Result<bool, d::WaitError> {
        unimplemented!()
    }

    unsafe fn set_event(&self, _event: &mut ()) -> Result<(), d::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn reset_event(&self, _event: &mut ()) -> Result<(), d::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn free_memory(&self, memory: n::Memory) {
        if let Some((buffer, _)) = memory.buffer {
            self.share.context.delete_buffer(buffer);
        }
    }

    unsafe fn create_query_pool(
        &self,
        ty: query::Type,
        _count: query::Id,
    ) -> Result<(), query::CreationError> {
        Err(query::CreationError::Unsupported(ty))
    }

    unsafe fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn get_query_pool_results(
        &self,
        _pool: &(),
        _queries: Range<query::Id>,
        _data: &mut [u8],
        _stride: buffer::Stride,
        _flags: query::ResultFlags,
    ) -> Result<bool, d::WaitError> {
        unimplemented!()
    }

    unsafe fn destroy_shader_module(&self, _: n::ShaderModule) {
        // Assumes compiled shaders are managed internally
    }

    unsafe fn destroy_render_pass(&self, _: n::RenderPass) {
        // Nothing to do
    }

    unsafe fn destroy_pipeline_layout(&self, _: n::PipelineLayout) {
        // Nothing to do
    }

    unsafe fn destroy_graphics_pipeline(&self, pipeline: n::GraphicsPipeline) {
        self.share.context.delete_program(pipeline.program);
    }

    unsafe fn destroy_compute_pipeline(&self, pipeline: n::ComputePipeline) {
        self.share.context.delete_program(pipeline.program);
    }

    unsafe fn destroy_framebuffer(&self, framebuffer: n::Framebuffer) {
        self.share.context.delete_framebuffer(framebuffer.raw);
    }

    unsafe fn destroy_buffer(&self, _buffer: n::Buffer) {
        // Nothing to do
    }

    unsafe fn destroy_buffer_view(&self, _: n::BufferView) {
        // Nothing to do
    }

    unsafe fn destroy_image(&self, image: n::Image) {
        let gl = &self.share.context;
        match image.object_type {
            n::ImageType::Renderbuffer { raw, .. } => gl.delete_renderbuffer(raw),
            n::ImageType::Texture { raw, .. } => gl.delete_texture(raw),
        }
    }

    unsafe fn destroy_image_view(&self, _image_view: n::ImageView) {
        // Nothing to do
    }

    unsafe fn destroy_sampler(&self, sampler: n::FatSampler) {
        let gl = &self.share.context;
        match sampler {
            n::FatSampler::Sampler(s) => gl.delete_sampler(s),
            _ => (),
        }
    }

    unsafe fn destroy_descriptor_pool(&self, _: n::DescriptorPool) {
        // Nothing to do
    }

    unsafe fn destroy_descriptor_set_layout(&self, _: n::DescriptorSetLayout) {
        // Nothing to do
    }

    unsafe fn destroy_fence(&self, fence: n::Fence) {
        match fence {
            n::Fence::Idle { .. } => {}
            n::Fence::Pending(sync) => {
                self.share.context.delete_sync(sync);
            }
        }
    }

    unsafe fn destroy_semaphore(&self, _: n::Semaphore) {
        // Nothing to do
    }

    unsafe fn destroy_event(&self, _event: ()) {
        unimplemented!()
    }

    fn wait_idle(&self) -> Result<(), d::OutOfMemory> {
        unsafe {
            self.share.context.finish();
        }
        Ok(())
    }

    unsafe fn set_image_name(&self, _image: &mut n::Image, _name: &str) {
        // TODO
    }

    unsafe fn set_buffer_name(&self, _buffer: &mut n::Buffer, _name: &str) {
        // TODO
    }

    unsafe fn set_command_buffer_name(
        &self,
        _command_buffer: &mut cmd::CommandBuffer,
        _name: &str,
    ) {
        // TODO
    }

    unsafe fn set_semaphore_name(&self, _semaphore: &mut n::Semaphore, _name: &str) {
        // TODO
    }

    unsafe fn set_fence_name(&self, _fence: &mut n::Fence, _name: &str) {
        // TODO
    }

    unsafe fn set_framebuffer_name(&self, _framebuffer: &mut n::Framebuffer, _name: &str) {
        // TODO
    }

    unsafe fn set_render_pass_name(&self, _render_pass: &mut n::RenderPass, _name: &str) {
        // TODO
    }

    unsafe fn set_descriptor_set_name(&self, _descriptor_set: &mut n::DescriptorSet, _name: &str) {
        // TODO
    }

    unsafe fn set_descriptor_set_layout_name(
        &self,
        _descriptor_set_layout: &mut n::DescriptorSetLayout,
        _name: &str,
    ) {
        // TODO
    }

    unsafe fn set_pipeline_layout_name(
        &self,
        _pipeline_layout: &mut n::PipelineLayout,
        _name: &str,
    ) {
        // TODO
    }

    unsafe fn set_display_power_state(
        &self,
        _display: &hal::display::Display<B>,
        _power_state: &hal::display::control::PowerState,
    ) -> Result<(), hal::display::control::DisplayControlError> {
        unimplemented!()
    }

    unsafe fn register_device_event(
        &self,
        _device_event: &hal::display::control::DeviceEvent,
        _fence: &mut <B as hal::Backend>::Fence,
    ) -> Result<(), hal::display::control::DisplayControlError> {
        unimplemented!()
    }

    unsafe fn register_display_event(
        &self,
        _display: &hal::display::Display<B>,
        _display_event: &hal::display::control::DisplayEvent,
        _fence: &mut <B as hal::Backend>::Fence,
    ) -> Result<(), hal::display::control::DisplayControlError> {
        unimplemented!()
    }

    fn start_capture(&self) {
        //TODO
    }

    fn stop_capture(&self) {
        //TODO
    }
}
