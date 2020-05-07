use arrayvec::ArrayVec;
use parking_lot::{Mutex, RwLock};
use spirv_cross::{glsl, spirv, ErrorCode as SpirvErrorCode};
use std::borrow::Borrow;
use std::cell::Cell;
use std::ops::Range;
use std::slice;
use std::sync::Arc;

use glow::HasContext;

use auxil::{spirv_cross_specialize_ast, FastHashMap};

use hal::{
    buffer,
    device as d,
    format::{Format, Swizzle},
    image as i,
    memory,
    pass,
    pool::CommandPoolCreateFlags,
    pso,
    query,
    queue,
    window::{Extent2D, SwapchainConfig},
};

use crate::{
    command as cmd,
    conv,
    info::LegacyFeatures,
    native as n,
    pool::{BufferMemory, CommandPool, OwnedBuffer},
    state,
    Backend as B,
    GlContainer,
    GlContext,
    MemoryUsage,
    Share,
    Starc,
    Surface,
    Swapchain,
};

/// Emit error during shader module creation. Used if we don't expect an error
/// but might panic due to an exception in SPIRV-Cross.
fn gen_unexpected_error(err: SpirvErrorCode) -> d::ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unexpected error".into(),
    };
    d::ShaderError::CompilationFailed(msg)
}

fn create_fbo_internal(
    share: &Starc<Share>,
) -> Option<<GlContext as glow::HasContext>::Framebuffer> {
    if share.private_caps.framebuffer {
        let gl = &share.context;
        let name = unsafe { gl.create_framebuffer() }.unwrap();
        info!("\tCreated frame buffer {:?}", name);
        Some(name)
    } else {
        None
    }
}

/// GL device.
#[derive(Debug)]
pub struct Device {
    pub(crate) share: Starc<Share>,
    features: hal::Features,
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
        }
    }

    pub fn create_shader_module_from_source(
        &self,
        shader: &str,
        stage: pso::Stage,
    ) -> Result<n::ShaderModule, d::ShaderError> {
        let gl = &self.share.context;

        let can_compute = self.share.limits.max_compute_work_group_count[0] != 0;
        let can_tessellate = self.share.limits.max_patch_size != 0;
        let target = match stage {
            pso::Stage::Vertex => glow::VERTEX_SHADER,
            pso::Stage::Hull if can_tessellate => glow::TESS_CONTROL_SHADER,
            pso::Stage::Domain if can_tessellate => glow::TESS_EVALUATION_SHADER,
            pso::Stage::Geometry => glow::GEOMETRY_SHADER,
            pso::Stage::Fragment => glow::FRAGMENT_SHADER,
            pso::Stage::Compute if can_compute => glow::COMPUTE_SHADER,
            _ => return Err(d::ShaderError::UnsupportedStage(stage)),
        };

        let name = unsafe { gl.create_shader(target) }.unwrap();
        unsafe {
            gl.shader_source(name, shader);
            gl.compile_shader(name);
        }
        info!("\tCompiled shader {:?}", name);
        if let Err(err) = self.share.check() {
            panic!("Error compiling shader: {:?}", err);
        }

        let compiled_ok = unsafe { gl.get_shader_compile_status(name) };
        let log = unsafe { gl.get_shader_info_log(name) };
        if compiled_ok {
            if !log.is_empty() {
                warn!("\tLog: {}", log);
            }
            Ok(n::ShaderModule::Raw(name))
        } else {
            Err(d::ShaderError::CompilationFailed(log))
        }
    }

    fn bind_target_compat(gl: &GlContainer, point: u32, attachment: u32, view: &n::ImageView) {
        match *view {
            n::ImageView::Renderbuffer(rb) => unsafe {
                gl.framebuffer_renderbuffer(point, attachment, glow::RENDERBUFFER, Some(rb));
            },
            n::ImageView::Texture(texture, textype, level) => unsafe {
                gl.bind_texture(textype, Some(texture));
                gl.framebuffer_texture_2d(point, attachment, textype, Some(texture), level as _);
            },
            n::ImageView::TextureLayer(texture, textype, level, layer) => unsafe {
                gl.bind_texture(textype, Some(texture));
                gl.framebuffer_texture_3d(
                    point,
                    attachment,
                    textype,
                    Some(texture),
                    level as _,
                    layer as _,
                );
            },
        }
    }

    fn bind_target(gl: &GlContainer, point: u32, attachment: u32, view: &n::ImageView) {
        match *view {
            n::ImageView::Renderbuffer(rb) => unsafe {
                gl.framebuffer_renderbuffer(point, attachment, glow::RENDERBUFFER, Some(rb));
            },
            n::ImageView::Texture(texture, _, level) => unsafe {
                gl.framebuffer_texture(point, attachment, Some(texture), level as _);
            },
            n::ImageView::TextureLayer(texture, _, level, layer) => unsafe {
                gl.framebuffer_texture_layer(
                    point,
                    attachment,
                    Some(texture),
                    level as _,
                    layer as _,
                );
            },
        }
    }

    fn parse_spirv(&self, raw_data: &[u32]) -> Result<spirv::Ast<glsl::Target>, d::ShaderError> {
        let module = spirv::Module::from_words(raw_data);

        spirv::Ast::parse(&module).map_err(|err| {
            let msg = match err {
                SpirvErrorCode::CompilationError(msg) => msg,
                SpirvErrorCode::Unhandled => "Unknown parsing error".into(),
            };
            d::ShaderError::CompilationFailed(msg)
        })
    }

    fn set_push_const_layout(
        &self,
        _ast: &mut spirv::Ast<glsl::Target>,
    ) -> Result<(), d::ShaderError> {
        Ok(())
    }

    fn translate_spirv(
        &self,
        ast: &mut spirv::Ast<glsl::Target>,
    ) -> Result<String, d::ShaderError> {
        let mut compile_options = glsl::CompilerOptions::default();
        // see version table at https://en.wikipedia.org/wiki/OpenGL_Shading_Language
        let is_embedded = self.share.info.shading_language.is_embedded;
        let version = self.share.info.shading_language.tuple();
        compile_options.version = if is_embedded {
            match version {
                (3, 00) => glsl::Version::V3_00Es,
                (1, 00) => glsl::Version::V1_00Es,
                other if other > (3, 00) => glsl::Version::V3_00Es,
                other => panic!("GLSL version is not recognized: {:?}", other),
            }
        } else {
            match version {
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
            }
        };
        compile_options.vertex.invert_y = !self.features.contains(hal::Features::NDC_Y_UP);
        debug!("SPIR-V options {:?}", compile_options);

        ast.set_compiler_options(&compile_options)
            .map_err(gen_unexpected_error)?;
        ast.compile().map_err(|err| {
            let msg = match err {
                SpirvErrorCode::CompilationError(msg) => msg,
                SpirvErrorCode::Unhandled => "Unknown compile error".into(),
            };
            d::ShaderError::CompilationFailed(msg)
        })
    }

    fn remap_bindings(
        &self,
        ast: &mut spirv::Ast<glsl::Target>,
        desc_remap_data: &mut n::DescRemapData,
        nb_map: &mut FastHashMap<String, (n::BindingTypes, pso::DescriptorBinding)>,
    ) {
        let res = ast.get_shader_resources().unwrap();
        self.remap_binding(
            ast,
            desc_remap_data,
            nb_map,
            &res.sampled_images,
            n::BindingTypes::Images,
        );
        self.remap_binding(
            ast,
            desc_remap_data,
            nb_map,
            &res.uniform_buffers,
            n::BindingTypes::UniformBuffers,
        );
        self.remap_binding(
            ast,
            desc_remap_data,
            nb_map,
            &res.storage_buffers,
            n::BindingTypes::StorageBuffers,
        );
    }

    fn remap_binding(
        &self,
        ast: &mut spirv::Ast<glsl::Target>,
        desc_remap_data: &mut n::DescRemapData,
        nb_map: &mut FastHashMap<String, (n::BindingTypes, pso::DescriptorBinding)>,
        all_res: &[spirv::Resource],
        btype: n::BindingTypes,
    ) {
        for res in all_res {
            let set = ast
                .get_decoration(res.id, spirv::Decoration::DescriptorSet)
                .unwrap();
            let binding = ast
                .get_decoration(res.id, spirv::Decoration::Binding)
                .unwrap();
            let nbs = desc_remap_data
                .get_binding(btype, set as _, binding)
                .unwrap();
            for nb in nbs {
                if self
                    .share
                    .legacy_features
                    .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
                {
                    ast.set_decoration(res.id, spirv::Decoration::Binding, *nb)
                        .unwrap()
                } else {
                    ast.unset_decoration(res.id, spirv::Decoration::Binding)
                        .unwrap();
                    assert!(nb_map.insert(res.name.clone(), (btype, *nb)).is_none());
                }
                ast.unset_decoration(res.id, spirv::Decoration::DescriptorSet)
                    .unwrap();
            }
        }
    }

    fn combine_separate_images_and_samplers(
        &self,
        ast: &mut spirv::Ast<glsl::Target>,
        desc_remap_data: &mut n::DescRemapData,
        nb_map: &mut FastHashMap<String, (n::BindingTypes, pso::DescriptorBinding)>,
    ) {
        let mut id_map =
            FastHashMap::<u32, (pso::DescriptorSetIndex, pso::DescriptorBinding)>::default();
        let res = ast.get_shader_resources().unwrap();
        self.populate_id_map(ast, &mut id_map, &res.separate_images);
        self.populate_id_map(ast, &mut id_map, &res.separate_samplers);

        for cis in ast.get_combined_image_samplers().unwrap() {
            let (set, binding) = id_map.get(&cis.image_id).unwrap();
            let nb = desc_remap_data.reserve_binding(n::BindingTypes::Images);
            desc_remap_data.insert_missing_binding(nb, n::BindingTypes::Images, *set, *binding);
            let (set, binding) = id_map.get(&cis.sampler_id).unwrap();
            desc_remap_data.insert_missing_binding(nb, n::BindingTypes::Images, *set, *binding);

            let new_name = "GFX_HAL_COMBINED_SAMPLER".to_owned()
                + "_"
                + &cis.sampler_id.to_string()
                + "_"
                + &cis.image_id.to_string()
                + "_"
                + &cis.combined_id.to_string();
            ast.set_name(cis.combined_id, &new_name).unwrap();
            if self
                .share
                .legacy_features
                .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
            {
                ast.set_decoration(cis.combined_id, spirv::Decoration::Binding, nb)
                    .unwrap()
            } else {
                ast.unset_decoration(cis.combined_id, spirv::Decoration::Binding)
                    .unwrap();
                assert!(nb_map
                    .insert(new_name, (n::BindingTypes::Images, nb))
                    .is_none())
            }
            ast.unset_decoration(cis.combined_id, spirv::Decoration::DescriptorSet)
                .unwrap();
        }
    }

    fn populate_id_map(
        &self,
        ast: &mut spirv::Ast<glsl::Target>,
        id_map: &mut FastHashMap<u32, (pso::DescriptorSetIndex, pso::DescriptorBinding)>,
        all_res: &[spirv::Resource],
    ) {
        for res in all_res {
            let set = ast
                .get_decoration(res.id, spirv::Decoration::DescriptorSet)
                .unwrap();
            let binding = ast
                .get_decoration(res.id, spirv::Decoration::Binding)
                .unwrap();
            assert!(id_map.insert(res.id, (set as _, binding)).is_none())
        }
    }

    fn compile_shader(
        &self,
        point: &pso::EntryPoint<B>,
        stage: pso::Stage,
        desc_remap_data: &mut n::DescRemapData,
        name_binding_map: &mut FastHashMap<String, (n::BindingTypes, pso::DescriptorBinding)>,
    ) -> n::Shader {
        assert_eq!(point.entry, "main");
        match *point.module {
            n::ShaderModule::Raw(raw) => {
                debug!("Can't remap bindings for raw shaders. Assuming they are already rebound.");
                raw
            }
            n::ShaderModule::Spirv(ref spirv) => {
                let mut ast = self.parse_spirv(spirv).unwrap();

                spirv_cross_specialize_ast(&mut ast, &point.specialization).unwrap();
                self.remap_bindings(&mut ast, desc_remap_data, name_binding_map);
                self.combine_separate_images_and_samplers(
                    &mut ast,
                    desc_remap_data,
                    name_binding_map,
                );
                self.set_push_const_layout(&mut ast).unwrap();

                let glsl = self.translate_spirv(&mut ast).unwrap();
                debug!("SPIRV-Cross generated shader:\n{}", glsl);
                let shader = match self.create_shader_module_from_source(&glsl, stage).unwrap() {
                    n::ShaderModule::Raw(raw) => raw,
                    _ => panic!("Unhandled"),
                };

                shader
            }
        }
    }
}

pub(crate) unsafe fn set_sampler_info<SetParamFloat, SetParamFloatVec, SetParamInt>(
    info: &i::SamplerDesc,
    features: &hal::Features,
    legacy_features: &LegacyFeatures,
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
    if legacy_features.contains(LegacyFeatures::SAMPLER_BORDER_COLOR) {
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
                    emulate_map_allocation: Cell::new(None),
                })
            }

            MemoryUsage::Image => {
                assert!(is_device_local_memory);
                Ok(n::Memory {
                    properties: memory::Properties::DEVICE_LOCAL,
                    buffer: None,
                    size,
                    map_flags: 0,
                    emulate_map_allocation: Cell::new(None),
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
        let limits = self.share.limits.into();
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

    unsafe fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        _dependencies: ID,
    ) -> Result<n::RenderPass, d::OutOfMemory>
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        let subpasses = subpasses
            .into_iter()
            .map(|subpass| {
                let subpass = subpass.borrow();
                assert!(
                    subpass.colors.len() <= self.share.limits.max_color_attachments,
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
            attachments: attachments
                .into_iter()
                .map(|attachment| attachment.borrow().clone())
                .collect::<Vec<_>>(),
            subpasses,
        })
    }

    unsafe fn create_pipeline_layout<IS, IR>(
        &self,
        layouts: IS,
        _: IR,
    ) -> Result<n::PipelineLayout, d::OutOfMemory>
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        let mut drd = n::DescRemapData::new();

        layouts.into_iter().enumerate().for_each(|(set, layout)| {
            layout.borrow().iter().for_each(|binding| {
                // DescriptorType -> Descriptor
                //
                // Sampler -> Sampler
                // Image -> SampledImage, StorageImage, InputAttachment
                // CombinedImageSampler -> CombinedImageSampler
                // Buffer -> UniformBuffer, StorageBuffer
                // UniformTexel -> UniformTexel
                // StorageTexel -> StorageTexel

                assert!(!binding.immutable_samplers); //TODO: Implement immutable_samplers
                use crate::pso::DescriptorType::*;
                match binding.ty {
                    Sampler
                    | Image {
                        ty:
                            pso::ImageDescriptorType::Sampled {
                                with_sampler: false,
                            },
                    } => {
                        // We need to figure out combos once we get the shaders, until then we
                        // do nothing
                    }
                    Image {
                        ty: pso::ImageDescriptorType::Sampled { with_sampler: true },
                    } => {
                        drd.insert_missing_binding_into_spare(
                            n::BindingTypes::Images,
                            set as _,
                            binding.binding,
                        );
                    }
                    Buffer {
                        ty,
                        format:
                            pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                    } => match ty {
                        pso::BufferDescriptorType::Uniform => {
                            drd.insert_missing_binding_into_spare(
                                n::BindingTypes::UniformBuffers,
                                set as _,
                                binding.binding,
                            );
                        }
                        pso::BufferDescriptorType::Storage { .. } => {
                            drd.insert_missing_binding_into_spare(
                                n::BindingTypes::StorageBuffers,
                                set as _,
                                binding.binding,
                            );
                        }
                    },
                    _ => unimplemented!(), // 6
                }
            })
        });

        Ok(n::PipelineLayout {
            desc_remap_data: Arc::new(RwLock::new(drd)),
        })
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

    unsafe fn merge_pipeline_caches<I>(&self, _: &(), _: I) -> Result<(), d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
    {
        //empty
        Ok(())
    }

    unsafe fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
        _cache: Option<&()>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let gl = &self.share.context;
        let share = &self.share;
        let desc = desc.borrow();
        let subpass = {
            let subpass = desc.subpass;
            match subpass.main_pass.subpasses.get(subpass.index as usize) {
                Some(sp) => sp,
                None => return Err(pso::CreationError::InvalidSubpass(subpass.index)),
            }
        };

        let (vertex_buffers, desc_attributes, input_assembler, vs, gs, hs, ds) =
            match desc.primitive_assembler {
                pso::PrimitiveAssembler::Vertex {
                    ref buffers,
                    ref attributes,
                    ref input_assembler,
                    ref vertex,
                    ref tessellation,
                    ref geometry,
                } => {
                    let (hs, ds) = if let Some(ts) = tessellation {
                        (Some(&ts.0), Some(&ts.1))
                    } else {
                        (None, None)
                    };

                    let mut vertex_buffers = Vec::new();
                    for vb in buffers {
                        while vertex_buffers.len() <= vb.binding as usize {
                            vertex_buffers.push(None);
                        }
                        vertex_buffers[vb.binding as usize] = Some(*vb);
                    }

                    (vertex_buffers, attributes, input_assembler, vertex, geometry, hs, ds)
                }
                pso::PrimitiveAssembler::Mesh { .. } => {
                    return Err(pso::CreationError::UnsupportedPipeline)
                }
            };

        let program = {
            let name = gl.create_program().unwrap();

            // Attach shaders to program
            let shaders = [
                (pso::Stage::Vertex, Some(vs)),
                (pso::Stage::Hull, hs),
                (pso::Stage::Domain, ds),
                (pso::Stage::Geometry, gs.as_ref()),
                (pso::Stage::Fragment, desc.fragment.as_ref()),
            ];

            let mut name_binding_map =
                FastHashMap::<String, (n::BindingTypes, pso::DescriptorBinding)>::default();
            let shader_names = &shaders
                .iter()
                .filter_map(|&(stage, point_maybe)| {
                    point_maybe.map(|point| {
                        let shader_name = self.compile_shader(
                            point,
                            stage,
                            &mut desc.layout.desc_remap_data.write(),
                            &mut name_binding_map,
                        );
                        gl.attach_shader(name, shader_name);
                        shader_name
                    })
                })
                .collect::<Vec<_>>();

            if !share.private_caps.program_interface && share.private_caps.frag_data_location {
                for i in 0 .. subpass.color_attachments.len() {
                    let color_name = format!("Target{}\0", i);
                    gl.bind_frag_data_location(name, i as u32, color_name.as_str());
                }
            }

            gl.link_program(name);
            info!("\tLinked program {:?}", name);
            if let Err(err) = share.check() {
                panic!("Error linking program: {:?}", err);
            }

            for shader_name in shader_names {
                gl.detach_shader(name, *shader_name);
                gl.delete_shader(*shader_name);
            }

            if !self
                .share
                .legacy_features
                .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
            {
                let gl = &self.share.context;
                gl.use_program(Some(name));
                for (bname, (btype, binding)) in name_binding_map.iter() {
                    match btype {
                        n::BindingTypes::Images => {
                            let loc = gl.get_uniform_location(name, bname);
                            gl.uniform_1_i32(loc, *binding as _);
                        }
                        n::BindingTypes::UniformBuffers => {
                            let index = gl.get_uniform_block_index(name, bname).unwrap();
                            gl.uniform_block_binding(name, index, *binding);
                        }
                        n::BindingTypes::StorageBuffers => {
                            let index = gl.get_shader_storage_block_index(name, bname).unwrap();
                            gl.shader_storage_block_binding(name, index, *binding);
                        }
                    }
                }
            }

            let linked_ok = gl.get_program_link_status(name);
            let log = gl.get_program_info_log(name);
            if linked_ok {
                if !log.is_empty() {
                    warn!("\tLog: {}", log);
                }
            } else {
                return Err(pso::CreationError::Shader(
                    d::ShaderError::CompilationFailed(log),
                ));
            }

            name
        };

        let patch_size = match input_assembler.primitive {
            pso::Primitive::PatchList(size) => Some(size as _),
            _ => None,
        };

        let mut uniforms = Vec::new();
        {
            let gl = &self.share.context;
            let count = gl.get_active_uniforms(program);

            let mut offset = 0;

            for uniform in 0 .. count {
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
        })
    }

    unsafe fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
        _cache: Option<&()>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        let gl = &self.share.context;
        let share = &self.share;

        let program = {
            let name = gl.create_program().unwrap();

            let mut name_binding_map =
                FastHashMap::<String, (n::BindingTypes, pso::DescriptorBinding)>::default();
            let shader = self.compile_shader(
                &desc.shader,
                pso::Stage::Compute,
                &mut desc.layout.desc_remap_data.write(),
                &mut name_binding_map,
            );

            gl.attach_shader(name, shader);
            gl.link_program(name);
            info!("\tLinked program {:?}", name);
            if let Err(err) = share.check() {
                panic!("Error linking program: {:?}", err);
            }

            gl.detach_shader(name, shader);
            gl.delete_shader(shader);

            if !self
                .share
                .legacy_features
                .contains(LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER)
            {
                let gl = &self.share.context;
                gl.use_program(Some(name));
                for (bname, (btype, binding)) in name_binding_map.iter() {
                    match btype {
                        n::BindingTypes::Images => {
                            let loc = gl.get_uniform_location(name, bname);
                            gl.uniform_1_i32(loc, *binding as _);
                        }
                        n::BindingTypes::UniformBuffers | n::BindingTypes::StorageBuffers => {
                            let index = gl.get_uniform_block_index(name, bname).unwrap();
                            gl.uniform_block_binding(name, index, *binding);
                        }
                    }
                }
            }

            let linked_ok = gl.get_program_link_status(name);
            let log = gl.get_program_info_log(name);
            if linked_ok {
                if !log.is_empty() {
                    warn!("\tLog: {}", log);
                }
            } else {
                return Err(pso::CreationError::Other);
            }

            name
        };

        Ok(n::ComputePipeline { program })
    }

    unsafe fn create_framebuffer<I>(
        &self,
        pass: &n::RenderPass,
        attachments: I,
        _extent: i::Extent,
    ) -> Result<n::FrameBuffer, d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<n::ImageView>,
    {
        if !self.share.private_caps.framebuffer {
            return Err(d::OutOfMemory::Host);
        }

        let attachments: Vec<_> = attachments
            .into_iter()
            .map(|at| at.borrow().clone())
            .collect();
        debug!("create_framebuffer {:?}", attachments);

        let gl = &self.share.context;
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
                if self.share.private_caps.framebuffer_texture {
                    Self::bind_target(gl, target, glow::DEPTH_STENCIL_ATTACHMENT, &attachments[depth_stencil]);
                } else {
                    Self::bind_target_compat(gl, target, glow::DEPTH_STENCIL_ATTACHMENT, &attachments[depth_stencil]);
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
                panic!(
                    "Error creating FBO: {:?} for {:?}", /* with attachments {:?}"*/
                    err, pass /*, attachments*/
                );
            }

            Some(name)
        }).collect();

        gl.bind_framebuffer(target, None);

        Ok(n::FrameBuffer { fbos })
    }

    unsafe fn create_shader_module(
        &self,
        raw_data: &[u32],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        Ok(n::ShaderModule::Spirv(raw_data.into()))
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
            &self.share.legacy_features,
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
    ) -> Result<n::Buffer, buffer::CreationError> {
        if !self
            .share
            .legacy_features
            .contains(LegacyFeatures::CONSTANT_BUFFER)
            && usage.contains(buffer::Usage::UNIFORM)
        {
            return Err(buffer::CreationError::UnsupportedUsage { usage });
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

        *buffer = n::Buffer::Bound {
            buffer: memory
                .buffer
                .expect("Improper memory type used for buffer memory")
                .0,
            range: offset .. offset + size,
        };

        Ok(())
    }

    unsafe fn map_memory(
        &self,
        memory: &n::Memory,
        segment: memory::Segment,
    ) -> Result<*mut u8, d::MapError> {
        let gl = &self.share.context;
        let caps = &self.share.private_caps;

        let offset = segment.offset;
        let size = segment.size.unwrap_or(memory.size - segment.offset);

        let (buffer, target) = memory.buffer.expect("cannot map image memory");
        let ptr = if caps.emulate_map {
            let ptr: *mut u8 = if let Some(ptr) = memory.emulate_map_allocation.get() {
                ptr
            } else {
                let ptr =
                    Box::into_raw(vec![0; memory.size as usize].into_boxed_slice()) as *mut u8;
                memory.emulate_map_allocation.set(Some(ptr));
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

    unsafe fn unmap_memory(&self, memory: &n::Memory) {
        let gl = &self.share.context;
        let (buffer, target) = memory.buffer.expect("cannot unmap image memory");

        gl.bind_buffer(target, Some(buffer));

        if self.share.private_caps.emulate_map {
            let ptr = memory.emulate_map_allocation.replace(None).unwrap();
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
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, memory::Segment)>,
    {
        let gl = &self.share.context;

        for i in ranges {
            let (mem, segment) = i.borrow();
            let (buffer, target) = mem.buffer.expect("cannot flush image memory");
            gl.bind_buffer(target, Some(buffer));

            let offset = segment.offset;
            let size = segment.size.unwrap_or(mem.size - segment.offset);

            if self.share.private_caps.emulate_map {
                let ptr = mem.emulate_map_allocation.get().unwrap();
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
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, memory::Segment)>,
    {
        let gl = &self.share.context;

        for i in ranges {
            let (mem, segment) = i.borrow();
            let (buffer, target) = mem.buffer.expect("cannot invalidate image memory");
            gl.bind_buffer(target, Some(buffer));

            let offset = segment.offset;
            let size = segment.size.unwrap_or(mem.size - segment.offset);

            if self.share.private_caps.emulate_map {
                let ptr = mem.emulate_map_allocation.get().unwrap();
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
        _view_caps: i::ViewCapabilities,
    ) -> Result<n::Image, i::CreationError> {
        let gl = &self.share.context;

        let desc = conv::describe_format(format).unwrap();
        let channel = format.base_format().1;

        let image = if num_levels > 1
            || usage.contains(i::Usage::STORAGE)
            || usage.contains(i::Usage::SAMPLED)
        {
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
                    } else {
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_MAX_LEVEL,
                            (num_levels - 1) as _,
                        );
                        let mut w = w;
                        let mut h = h;
                        for i in 0 .. num_levels {
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
                            w = std::cmp::max(w / 2, 1);
                            h = std::cmp::max(h / 2, 1);
                        }
                    }
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
                    } else {
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D_ARRAY,
                            glow::TEXTURE_MAX_LEVEL,
                            (num_levels - 1) as _,
                        );
                        let mut w = w;
                        let mut h = h;
                        for i in 0 .. num_levels {
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
                            w = std::cmp::max(w / 2, 1);
                            h = std::cmp::max(h / 2, 1);
                        }
                    }
                    glow::TEXTURE_2D_ARRAY
                }
                _ => unimplemented!(),
            };
            n::ImageKind::Texture {
                texture: name,
                target,
                format: desc.tex_external,
                pixel_type: desc.data_type,
            }
        } else {
            let name = gl.create_renderbuffer().unwrap();
            match kind {
                i::Kind::D2(w, h, 1, 1) => {
                    gl.bind_renderbuffer(glow::RENDERBUFFER, Some(name));
                    gl.renderbuffer_storage(glow::RENDERBUFFER, desc.tex_internal, w as _, h as _);
                }
                _ => unimplemented!(),
            };
            n::ImageKind::Renderbuffer {
                renderbuffer: name,
                format: desc.tex_external,
            }
        };

        let surface_desc = format.base_format().0.desc();
        let bytes_per_texel = surface_desc.bits / 8;
        let ext = kind.extent();
        let size = (ext.width * ext.height * ext.depth) as u64 * bytes_per_texel as u64;
        let type_mask = self.share.image_memory_type_mask();

        if let Err(err) = self.share.check() {
            panic!(
                "Error creating image: {:?} for kind {:?} of {:?}",
                err, kind, format
            );
        }

        Ok(n::Image {
            kind: image,
            channel,
            requirements: memory::Requirements {
                size,
                alignment: 1,
                type_mask,
            },
        })
    }

    unsafe fn get_image_requirements(&self, unbound: &n::Image) -> memory::Requirements {
        unbound.requirements
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        _image: &n::Image,
        _sub: i::Subresource,
    ) -> i::SubresourceFootprint {
        unimplemented!()
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
        _kind: i::ViewKind,
        _format: Format,
        swizzle: Swizzle,
        range: i::SubresourceRange,
    ) -> Result<n::ImageView, i::ViewCreationError> {
        //TODO: check if `layers.end` covers all the layers
        let level = range.levels.start;
        assert_eq!(level + 1, range.levels.end);
        //assert_eq!(format, image.format);
        assert_eq!(swizzle, Swizzle::NO);
        //TODO: check format
        match image.kind {
            n::ImageKind::Renderbuffer { renderbuffer, .. } => {
                if range.levels.start == 0 && range.layers.start == 0 {
                    Ok(n::ImageView::Renderbuffer(renderbuffer))
                } else if level != 0 {
                    Err(i::ViewCreationError::Level(level)) //TODO
                } else {
                    Err(i::ViewCreationError::Layer(i::LayerError::OutOfBounds(
                        range.layers,
                    )))
                }
            }
            n::ImageKind::Texture {
                texture, target, ..
            } => {
                //TODO: check that `level` exists
                if range.layers.start == 0 {
                    Ok(n::ImageView::Texture(texture, target, level))
                } else if range.layers.start + 1 == range.layers.end {
                    Ok(n::ImageView::TextureLayer(
                        texture,
                        target,
                        level,
                        range.layers.start,
                    ))
                } else {
                    Err(i::ViewCreationError::Layer(i::LayerError::OutOfBounds(
                        range.layers,
                    )))
                }
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
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        Ok(n::DescriptorPool {})
    }

    unsafe fn create_descriptor_set_layout<I, J>(
        &self,
        layout: I,
        _: J,
    ) -> Result<n::DescriptorSetLayout, d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<n::FatSampler>,
    {
        // Just return it
        Ok(layout.into_iter().map(|l| l.borrow().clone()).collect())
    }

    unsafe fn write_descriptor_sets<'a, I, J>(&self, writes: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, B, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, B>>,
    {
        for mut write in writes {
            let set = &mut write.set;
            let mut bindings = set.bindings.lock();
            let binding = write.binding;

            for descriptor in write.descriptors {
                match descriptor.borrow() {
                    pso::Descriptor::Buffer(buffer, ref sub) => {
                        let (raw_buffer, buffer_range) = buffer.as_bound();
                        let range = crate::resolve_sub_range(sub, buffer_range);

                        let ty = set.layout[binding as usize].ty;
                        let ty = match ty {
                            pso::DescriptorType::Buffer { ty, .. } => match ty {
                                pso::BufferDescriptorType::Uniform => {
                                    n::BindingTypes::UniformBuffers
                                }
                                pso::BufferDescriptorType::Storage { .. } => {
                                    n::BindingTypes::StorageBuffers
                                }
                            },
                            _ => panic!("Can't write buffer into descriptor of type {:?}", ty),
                        };

                        bindings.push(n::DescSetBindings::Buffer {
                            ty,
                            binding,
                            buffer: raw_buffer,
                            offset: range.start as i32,
                            size: (range.end - range.start) as i32,
                        });
                    }
                    pso::Descriptor::CombinedImageSampler(view, _layout, sampler) => {
                        match view {
                            n::ImageView::Texture(tex, textype, _)
                            | n::ImageView::TextureLayer(tex, textype, _, _) => {
                                bindings.push(n::DescSetBindings::Texture(binding, *tex, *textype))
                            }
                            n::ImageView::Renderbuffer(_) => unimplemented!(),
                        }
                        match sampler {
                            n::FatSampler::Sampler(sampler) => {
                                bindings.push(n::DescSetBindings::Sampler(binding, *sampler))
                            }
                            n::FatSampler::Info(info) => bindings
                                .push(n::DescSetBindings::SamplerDesc(binding, info.clone())),
                        }
                    }
                    pso::Descriptor::Image(view, _layout) => match view {
                        n::ImageView::Texture(tex, textype, _)
                        | n::ImageView::TextureLayer(tex, textype, _, _) => {
                            bindings.push(n::DescSetBindings::Texture(binding, *tex, *textype))
                        }
                        n::ImageView::Renderbuffer(_) => panic!(
                            "Texture was created with only render target usage which is invalid."
                        ),
                    },
                    pso::Descriptor::Sampler(sampler) => match sampler {
                        n::FatSampler::Sampler(sampler) => {
                            bindings.push(n::DescSetBindings::Sampler(binding, *sampler))
                        }
                        n::FatSampler::Info(info) => {
                            bindings.push(n::DescSetBindings::SamplerDesc(binding, info.clone()))
                        }
                    },
                    pso::Descriptor::TexelBuffer(_view) => unimplemented!(),
                }
            }
        }
    }

    unsafe fn copy_descriptor_sets<'a, I>(&self, copies: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, B>>,
    {
        for _copy in copies {
            unimplemented!()
        }
    }

    fn create_semaphore(&self) -> Result<n::Semaphore, d::OutOfMemory> {
        Ok(n::Semaphore)
    }

    fn create_fence(&self, signaled: bool) -> Result<n::Fence, d::OutOfMemory> {
        let cell = Cell::new(n::FenceInner::Idle { signaled });
        Ok(n::Fence(cell))
    }

    unsafe fn reset_fence(&self, fence: &n::Fence) -> Result<(), d::OutOfMemory> {
        fence.0.replace(n::FenceInner::Idle { signaled: false });
        Ok(())
    }

    unsafe fn wait_for_fence(
        &self,
        fence: &n::Fence,
        timeout_ns: u64,
    ) -> Result<bool, d::OomOrDeviceLost> {
        // TODO:
        // This can be called by multiple objects wanting to ensure they have exclusive
        // access to a resource. How much does this call costs ? The status of the fence
        // could be cached to avoid calling this more than once (in core or in the backend ?).
        let gl = &self.share.context;
        match fence.0.get() {
            n::FenceInner::Idle { signaled } => {
                if !signaled {
                    warn!(
                        "Fence ptr {:?} is not pending, waiting not possible",
                        fence.0.as_ptr()
                    );
                }
                Ok(signaled)
            }
            n::FenceInner::Pending(Some(sync)) => {
                // TODO: Could `wait_sync` be used here instead?
                match gl.client_wait_sync(sync, glow::SYNC_FLUSH_COMMANDS_BIT, timeout_ns as i32) {
                    glow::TIMEOUT_EXPIRED => Ok(false),
                    glow::WAIT_FAILED => {
                        if let Err(err) = self.share.check() {
                            error!("Error when waiting on fence: {:?}", err);
                        }
                        Ok(false)
                    }
                    glow::CONDITION_SATISFIED | glow::ALREADY_SIGNALED => {
                        fence.0.set(n::FenceInner::Idle { signaled: true });
                        Ok(true)
                    }
                    _ => unreachable!(),
                }
            }
            n::FenceInner::Pending(None) => {
                // No sync capability, we fallback to waiting for *everything* to finish
                gl.flush();
                fence.0.set(n::FenceInner::Idle { signaled: true });
                Ok(true)
            }
        }
    }

    #[cfg(wasm)]
    unsafe fn wait_for_fences<I>(
        &self,
        fences: I,
        wait: d::WaitFor,
        timeout_ns: u64,
    ) -> Result<bool, d::OomOrDeviceLost>
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        let performance = web_sys::window().unwrap().performance().unwrap();
        let start = performance.now();
        let get_elapsed = || ((performance.now() - start) * 1_000_000.0) as u64;

        match wait {
            d::WaitFor::All => {
                for fence in fences {
                    if !self.wait_for_fence(fence.borrow(), 0)? {
                        let elapsed_ns = get_elapsed();
                        if elapsed_ns > timeout_ns {
                            return Ok(false);
                        }
                        if !self.wait_for_fence(fence.borrow(), timeout_ns - elapsed_ns)? {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
            d::WaitFor::Any => {
                const FENCE_WAIT_NS: u64 = 100_000;

                let fences: Vec<_> = fences.into_iter().collect();
                loop {
                    for fence in &fences {
                        if self.wait_for_fence(fence.borrow(), FENCE_WAIT_NS)? {
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
        Ok(match fence.0.get() {
            n::FenceInner::Pending(Some(sync)) => {
                self.share.context.get_sync_status(sync) == glow::SIGNALED
            }
            n::FenceInner::Pending(None) => false,
            n::FenceInner::Idle { signaled } => signaled,
        })
    }

    fn create_event(&self) -> Result<(), d::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn get_event_status(&self, _event: &()) -> Result<bool, d::OomOrDeviceLost> {
        unimplemented!()
    }

    unsafe fn set_event(&self, _event: &()) -> Result<(), d::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn reset_event(&self, _event: &()) -> Result<(), d::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn free_memory(&self, memory: n::Memory) {
        if let Some((buffer, _)) = memory.buffer {
            self.share.context.delete_buffer(buffer);
        }
    }

    unsafe fn create_query_pool(
        &self,
        _ty: query::Type,
        _count: query::Id,
    ) -> Result<(), query::CreationError> {
        unimplemented!()
    }

    unsafe fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn get_query_pool_results(
        &self,
        _pool: &(),
        _queries: Range<query::Id>,
        _data: &mut [u8],
        _stride: buffer::Offset,
        _flags: query::ResultFlags,
    ) -> Result<bool, d::OomOrDeviceLost> {
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

    unsafe fn destroy_framebuffer(&self, frame_buffer: n::FrameBuffer) {
        let gl = &self.share.context;
        for f in frame_buffer.fbos {
            if let Some(f) = f {
                gl.delete_framebuffer(f);
            }
        }
    }

    unsafe fn destroy_buffer(&self, _buffer: n::Buffer) {
        // Nothing to do
    }

    unsafe fn destroy_buffer_view(&self, _: n::BufferView) {
        // Nothing to do
    }

    unsafe fn destroy_image(&self, image: n::Image) {
        let gl = &self.share.context;
        match image.kind {
            n::ImageKind::Renderbuffer { renderbuffer, .. } => gl.delete_renderbuffer(renderbuffer),
            n::ImageKind::Texture { texture, .. } => gl.delete_texture(texture),
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
        match fence.0.get() {
            n::FenceInner::Pending(Some(sync)) => {
                self.share.context.delete_sync(sync);
            }
            _ => {}
        }
    }

    unsafe fn destroy_semaphore(&self, _: n::Semaphore) {
        // Nothing to do
    }

    unsafe fn destroy_event(&self, _event: ()) {
        unimplemented!()
    }

    unsafe fn create_swapchain(
        &self,
        surface: &mut Surface,
        config: SwapchainConfig,
        _old_swapchain: Option<Swapchain>,
    ) -> Result<(Swapchain, Vec<n::Image>), hal::window::CreationError> {
        let gl = &self.share.context;

        #[cfg(wgl)]
        let context = {
            use crate::window::wgl::PresentContext;

            let context = PresentContext::new(surface, &self.share.instance_context);
            context.make_current();
            context
        };

        let mut fbos = ArrayVec::new();
        let mut images = Vec::new();

        for _ in 0 .. config.image_count {
            let fbo = gl.create_framebuffer().unwrap();
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            fbos.push(fbo);

            let Extent2D { width, height } = config.extent;
            let image = self
                .create_image(
                    i::Kind::D2(width, height, config.image_layers, 1),
                    1,
                    config.format,
                    i::Tiling::Optimal,
                    config.image_usage,
                    i::ViewCapabilities::empty(),
                )
                .unwrap();

            match image.kind {
                n::ImageKind::Renderbuffer { renderbuffer, .. } => {
                    gl.framebuffer_renderbuffer(
                        glow::FRAMEBUFFER,
                        glow::COLOR_ATTACHMENT0,
                        glow::RENDERBUFFER,
                        Some(renderbuffer),
                    );
                }
                n::ImageKind::Texture {
                    texture, target, ..
                } => {
                    if self.share.private_caps.framebuffer_texture {
                        gl.framebuffer_texture(
                            glow::FRAMEBUFFER,
                            glow::COLOR_ATTACHMENT0,
                            Some(texture),
                            0,
                        );
                    } else {
                        gl.bind_texture(target, Some(texture));
                        gl.framebuffer_texture_2d(
                            glow::FRAMEBUFFER,
                            glow::COLOR_ATTACHMENT0,
                            target,
                            Some(texture),
                            0,
                        );
                    }
                }
            }

            images.push(image);
        }

        // Web
        #[cfg(wasm)]
        let _ = surface;
        #[cfg(wasm)]
        let swapchain = Swapchain {
            fbos,
            extent: config.extent,
        };

        // Glutin
        #[cfg(glutin)]
        let swapchain = Swapchain {
            fbos,
            extent: config.extent,
            context: {
                surface.context().resize(glutin::dpi::PhysicalSize::new(
                    config.extent.width,
                    config.extent.height,
                ));
                surface.context.clone()
            },
        };

        // Surfman
        #[cfg(surfman)]
        let swapchain = Swapchain {
            fbos,
            extent: config.extent,
            // TODO: Resize the context to the extent
            context: surface.context.clone(),
        };

        // WGL
        #[cfg(wgl)]
        let swapchain = {
            self.share.instance_context.make_current();
            Swapchain {
                fbos,
                context,
                extent: config.extent,
            }
        };

        // Dummy
        #[cfg(dummy)]
        let swapchain = Swapchain {
            extent: {
                let _ = surface;
                config.extent
            },
            fbos,
        };

        Ok((swapchain, images))
    }

    unsafe fn destroy_swapchain(&self, _swapchain: Swapchain) {
        // Nothing to do
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

    unsafe fn set_framebuffer_name(&self, _framebuffer: &mut n::FrameBuffer, _name: &str) {
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
}
