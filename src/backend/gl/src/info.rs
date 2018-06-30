use std::collections::HashSet;
use std::{ffi, fmt, mem, str};
use gl;
use hal::{Features, Limits};
use std::ops::RangeInclusive;

/// A version number for a specific component of an OpenGL implementation
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    pub is_embedded: bool,
    pub major: u32,
    pub minor: u32,
    pub revision: Option<u32>,
    pub vendor_info: &'static str,
}

impl Version {
    /// Create a new OpenGL version number
    pub fn new(
        major: u32, minor: u32, revision: Option<u32>,
        vendor_info: &'static str,
    ) -> Self {
        Version {
            is_embedded: false,
            major: major,
            minor: minor,
            revision: revision,
            vendor_info: vendor_info,
        }
    }
    /// Create a new OpenGL ES version number
    pub fn new_embedded(major: u32, minor: u32, vendor_info: &'static str) -> Self {
        Version {
            is_embedded: true,
            major: major,
            minor: minor,
            revision: None,
            vendor_info: vendor_info,
        }
    }

    /// Get a tuple of (major, minor) versions
    pub fn tuple(&self) -> (u32, u32) {
        (self.major, self.minor)
    }

    /// According to the OpenGL specification, the version information is
    /// expected to follow the following syntax:
    ///
    /// ~~~bnf
    /// <major>       ::= <number>
    /// <minor>       ::= <number>
    /// <revision>    ::= <number>
    /// <vendor-info> ::= <string>
    /// <release>     ::= <major> "." <minor> ["." <release>]
    /// <version>     ::= <release> [" " <vendor-info>]
    /// ~~~
    ///
    /// Note that this function is intentionally lenient in regards to parsing,
    /// and will try to recover at least the first two version numbers without
    /// resulting in an `Err`.
    pub fn parse(mut src: &'static str) -> Result<Version, &'static str> {
        let es_sig = " ES ";
        let is_es = match src.rfind(es_sig) {
            Some(pos) => {
                src = &src[pos + es_sig.len() ..];
                true
            },
            None => false,
        };
        let (version, vendor_info) = match src.find(' ') {
            Some(i) => (&src[..i], &src[i+1..]),
            None => (src, ""),
        };

        // TODO: make this even more lenient so that we can also accept
        // `<major> "." <minor> [<???>]`
        let mut it = version.split('.');
        let major = it.next().and_then(|s| s.parse().ok());
        let minor = it.next().and_then(|s| s.parse().ok());
        let revision = it.next().and_then(|s| s.parse().ok());

        match (major, minor, revision) {
            (Some(major), Some(minor), revision) => Ok(Version {
                is_embedded: is_es,
                major: major,
                minor: minor,
                revision: revision,
                vendor_info: vendor_info,
            }),
            (_, _, _) => Err(src),
        }
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (self.major, self.minor, self.revision, self.vendor_info) {
            (major, minor, Some(revision), "") =>
                write!(f, "{}.{}.{}", major, minor, revision),
            (major, minor, None, "") =>
                write!(f, "{}.{}", major, minor),
            (major, minor, Some(revision), vendor_info) =>
                write!(f, "{}.{}.{}, {}", major, minor, revision, vendor_info),
            (major, minor, None, vendor_info) =>
                write!(f, "{}.{}, {}", major, minor, vendor_info),
        }
    }
}

const EMPTY_STRING: &'static str = "";

/// Get a statically allocated string from the implementation using
/// `glGetString`. Fails if it `GLenum` cannot be handled by the
/// implementation's `gl.GetString` function.
fn get_string(gl: &gl::Gl, name: gl::types::GLenum) -> &'static str {
    let ptr = unsafe { gl.GetString(name) } as *const i8;
    if !ptr.is_null() {
        // This should be safe to mark as statically allocated because
        // GlGetString only returns static strings.
        unsafe { c_str_as_static_str(ptr) }
    } else {
        error!("Invalid GLenum passed to `get_string`: {:x}", name);
        EMPTY_STRING
    }
}

fn get_u32(gl: &gl::Gl, name: gl::types::GLenum) -> u32 {
    let mut value = 0 as gl::types::GLint;
    unsafe { gl.GetIntegerv(name, &mut value) };
    value as u32
}

fn get_i32(gl: &gl::Gl, name: gl::types::GLenum) -> i32 {
    let mut value = 0 as gl::types::GLint;
    unsafe { gl.GetIntegerv(name, &mut value) };
    value as i32
}

fn get_indexed_u32(gl: &gl::Gl, name: gl::types::GLenum, index: u32) -> u32 {
    let mut value = 0 as gl::types::GLint;
    unsafe { gl.GetIntegeri_v(name, index, &mut value) };
    value as u32
}

fn get_u32_pair(gl: &gl::Gl, name: gl::types::GLenum) -> [u32; 2] {
    let mut value = [0 as gl::types::GLint; 2];
    unsafe { gl.GetIntegerv(name, &mut value) };
    [value[0] as u32, value[1] as u32]
}

fn get_f32(gl: &gl::Gl, name: gl::types::GLenum) -> f32 {
    let mut value = 0 as gl::types::GLfloat;
    unsafe { gl.GetFloatv(name, &mut value) };
    value as f32
}

fn get_f32_range(gl: &gl::Gl, name: gl::types::GLenum) -> RangeInclusive<f32> {
    let mut value = [0 as gl::types::GLfloat; 2];
    unsafe { gl.GetFloatv(name, &mut value) };
    (value[0] as f32)..=(value[1] as f32)
}

unsafe fn c_str_as_static_str(c_str: *const i8) -> &'static str {
    //TODO: avoid transmuting
    mem::transmute(str::from_utf8(ffi::CStr::from_ptr(c_str as *const _).to_bytes()).unwrap())
}

/// A unique platform identifier that does not change between releases
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PlatformName {
    /// The company responsible for the OpenGL implementation
    pub vendor: &'static str,
    /// The name of the renderer
    pub renderer: &'static str,
}

impl PlatformName {
    fn get(gl: &gl::Gl) -> Self {
        PlatformName {
            vendor: get_string(gl, gl::VENDOR),
            renderer: get_string(gl, gl::RENDERER),
        }
    }
}

/// Private capabilities that don't need to be exposed.
/// The affect the implementation code paths but not the
/// provided API surface.
#[derive(Debug)]
pub struct PrivateCaps {
    /// VAO support
    pub vertex_array: bool,
    /// FBO support
    pub framebuffer: bool,
    /// FBO support to call `glFramebufferTexture`
    pub framebuffer_texture: bool,
    /// Can bind a buffer to a different target than was
    /// used upon the buffer creation/initialization
    pub buffer_role_change: bool,
    pub buffer_storage: bool,
    pub image_storage: bool,
    pub clear_buffer: bool,
    pub program_interface: bool,
    pub frag_data_location: bool,
    pub sync: bool,
    /// Can map memory
    pub map: bool,
    /// Indicates if we only have support via the EXT.
    pub sampler_anisotropy_ext: bool,
}

/// OpenGL implementation information
#[derive(Debug)]
pub struct Info {
    /// The platform identifier
    pub platform_name: PlatformName,
    /// The OpenGL API version number
    pub version: Version,
    /// The GLSL version number
    pub shading_language: Version,
    /// The extensions supported by the implementation
    pub extensions: HashSet<&'static str>,
}

bitflags! {
    /// Flags for features that are required for Vulkan but may not
    /// be supported by legacy backends (GL/DX11).
    pub struct LegacyFeatures: u16 {
        /// Support indirect drawing and dispatching.
        const INDIRECT_EXECUTION = 0x0001;
        /// Support instanced drawing.
        const DRAW_INSTANCED = 0x0002;
        /// Support offsets for instanced drawing with base instance.
        const DRAW_INSTANCED_BASE = 0x0004;
        /// Support indexed drawing with base vertex.
        const DRAW_INDEXED_BASE = 0x0008;
        /// Support indexed, instanced drawing.
        const DRAW_INDEXED_INSTANCED = 0x0010;
        /// Support indexed, instanced drawing with base vertex only.
        const DRAW_INDEXED_INSTANCED_BASE_VERTEX = 0x0020;
        /// Support indexed, instanced drawing with base vertex and instance.
        const DRAW_INDEXED_INSTANCED_BASE = 0x0040;
        /// Support base vertex offset for indexed drawing.
        const VERTEX_BASE = 0x0080;
        /// Support sRGB textures and rendertargets.
        const SRGB_COLOR = 0x0100;
        /// Support constant buffers.
        const CONSTANT_BUFFER = 0x0200;
        /// Support unordered-access views.
        const UNORDERED_ACCESS_VIEW = 0x0400;
        /// Support accelerated buffer copy.
        const COPY_BUFFER = 0x0800;
        /// Support separation of textures and samplers.
        const SAMPLER_OBJECTS = 0x1000;
        /// Support sampler LOD bias.
        const SAMPLER_LOD_BIAS = 0x2000;
        /// Support setting border texel colors.
        const SAMPLER_BORDER_COLOR = 0x4000;
        /// No explicit layouts in shader support
        const EXPLICIT_LAYOUTS_IN_SHADER = 0x8000;
    }
}

#[derive(Copy, Clone)]
pub enum Requirement {
    Core(u32,u32),
    Es(u32, u32),
    Ext(&'static str),
}

impl Info {
    fn get(gl: &gl::Gl) -> Info {
        let platform_name = PlatformName::get(gl);
        let version = Version::parse(get_string(gl, gl::VERSION)).unwrap();
        let shading_language = Version::parse(get_string(gl, gl::SHADING_LANGUAGE_VERSION)).unwrap();
        let extensions = if version >= Version::new(3, 0, None, "") {
            let num_exts = get_u32(gl, gl::NUM_EXTENSIONS) as gl::types::GLuint;
            (0..num_exts)
                .map(|i| unsafe { c_str_as_static_str(gl.GetStringi(gl::EXTENSIONS, i) as *const i8) })
                .collect()
        } else {
            // Fallback
            get_string(gl, gl::EXTENSIONS).split(' ').collect()
        };
        Info {
            platform_name: platform_name,
            version: version,
            shading_language: shading_language,
            extensions: extensions,
        }
    }

    pub fn is_version_supported(&self, major: u32, minor: u32) -> bool {
        !self.version.is_embedded && self.version >= Version::new(major, minor, None, "")
    }

    pub fn is_embedded_version_supported(&self, major: u32, minor: u32) -> bool {
        self.version.is_embedded && self.version >= Version::new(major, minor, None, "")
    }

    /// Returns `true` if the implementation supports the extension
    pub fn is_extension_supported(&self, s: &'static str) -> bool {
        self.extensions.contains(&s)
    }

    pub fn is_version_or_extension_supported(&self, major: u32, minor: u32, ext: &'static str) -> bool {
        self.is_version_supported(major, minor) || self.is_extension_supported(ext)
    }

    pub fn is_any_extension_supported(&self, exts: &[&'static str]) -> bool {
        exts.iter().any(|e| self.extensions.contains(e))
    }

    pub fn is_supported(&self, requirements: &[Requirement]) -> bool {
        use self::Requirement::*;
        requirements.iter().any(|r| {
            match *r {
                Core(major, minor) => self.is_version_supported(major, minor),
                Es(major, minor) => self.is_embedded_version_supported(major, minor),
                Ext(extension) => self.is_extension_supported(extension),
            }
        })
    }
}

/// Load the information pertaining to the driver and the corresponding device
/// capabilities.
pub fn query_all(gl: &gl::Gl) -> (Info, Features, LegacyFeatures, Limits, PrivateCaps) {
    use self::Requirement::*;
    let info = Info::get(gl);

    let mut limits = Limits {
        max_1d_texture_size: get_u32(gl, gl::MAX_TEXTURE_SIZE),
        max_2d_texture_size: get_u32(gl, gl::MAX_TEXTURE_SIZE),
        max_3d_texture_size: get_u32(gl, gl::MAX_3D_TEXTURE_SIZE),
        max_cube_texture_size: get_u32(gl, gl::MAX_CUBE_MAP_TEXTURE_SIZE),
        max_array_layers: get_u32(gl, gl::MAX_ARRAY_TEXTURE_LAYERS),
        max_texel_buffer_elements: 0,
        max_uniform_buffer_range: get_u32(gl, gl::MAX_UNIFORM_BUFFER_BINDINGS),
        max_storage_buffer_range: 0,
        max_push_constant_size: 0,
        max_memory_allocation_count: 0,
        max_sampler_count: 0,
        buffer_granularity: 0,
        sparse_address_space_size: 0,
        max_bound_descriptor_sets: 0,
        max_samplers_per_stage: 0,
        max_uniform_buffers_per_stage: 0,
        max_storage_buffers_per_stage: 0,
        max_sampled_images_per_stage: 0,
        max_storage_images_per_stage: 0,
        max_input_attachments_per_stage: 0,
        max_resources_per_stage: 0,
        max_samplers_per_pipeline: 0,
        max_uniform_buffers_per_pipeline: 0,
        max_dynamic_uniform_buffers_per_pipeline: 0,
        max_storage_buffers_per_pipeline: 0,
        max_dynamic_storage_buffers_per_pipeline: 0,
        max_sampled_images_per_pipeline: 0,
        max_storage_images_per_pipeline: 0,
        max_input_attachments_per_pipeline: 0,
        max_vertex_input_attributes: 0, //MAX_VERTEX_ATTRIBS
        max_vertex_input_bindings: 0, //MAX_VERTEX_ATTRIB_BINDINGS
        max_vertex_input_attribute_offset: 0, //MAX_VERTEX_ATTRIB_RELATIVE_OFFSET           //MAX_ELEMENTS_VERTICES & MAX_ELEMENTS_INDICES
        max_vertex_input_binding_stride: 0, //MAX_VERTEX_ATTRIB_STRIDE                      //MAX_VERTEX_UNIFORM_COMPONENTS/VECTORS/BLOCKS
        max_vertex_output_components: get_u32(gl, gl::MAX_VERTEX_OUTPUT_COMPONENTS),//MAX_VERTEX_TEXTURE_IMAGE_UNITS
        max_generation_level: get_u32(gl, gl::MAX_TESS_GEN_LEVEL),
        max_patch_size: 0,
        max_tesselation_control_input_components_per_vertex: get_u32(gl, gl::MAX_TESS_CONTROL_INPUT_COMPONENTS),
        max_tesselation_control_output_components_per_vertex: get_u32(gl, gl::MAX_TESS_CONTROL_OUTPUT_COMPONENTS),
        max_tesselation_control_output_components_per_patch: get_u32(gl, gl::MAX_TESS_PATCH_COMPONENTS),
        max_tesselation_control_output_components_total: get_u32(gl, gl::MAX_TESS_CONTROL_TOTAL_OUTPUT_COMPONENTS),
        max_tesselation_evaluation_input_components: get_u32(gl, gl::MAX_TESS_EVALUATION_INPUT_COMPONENTS),
        max_tesselation_evaluation_output_components: get_u32(gl, gl::MAX_TESS_EVALUATION_OUTPUT_COMPONENTS),
        max_geometry_shader_invocations: get_u32(gl, gl::MAX_GEOMETRY_SHADER_INVOCATIONS),
        max_geometry_input_components: get_u32(gl, gl::MAX_GEOMETRY_INPUT_COMPONENTS),
        max_geometry_output_components: get_u32(gl, gl::MAX_GEOMETRY_OUTPUT_COMPONENTS),
        max_geometry_output_vertices: get_u32(gl, gl::MAX_GEOMETRY_OUPTUT_VERTICES),
        max_geometry_output_components_total: get_u32(gl, gl::MAX_GEOMETRY_TOTAL_OUTPUT_COMPONENTS),
        max_fragment_input_components: get_u32(gl, gl::MAX_FRAGMENT_INPUT_COMPONENTS),
        max_fragment_output_attachments: 0, //MAX_DRAW_BUFFERS
        max_fragment_output_attachment_dual_src: 0, //MAX_DUAL_SOURCE_DRAW_BUFFERS
        max_fragment_output_resources: 0,
        max_compute_shared_memory_size: get_u32(gl, gl::MAX_COMPUTE_SHARED_MEMORY_SIZE),
        max_compute_group_count: [0; 3],
        max_compute_group_invocations: 0,
        max_compute_group_size: [0; 3],
        sub_pixel_precision_bits: get_u32(gl, gl::SUBPIXEL_BITS),
        sub_texel_precision_bits: 0,
        mipmap_precision_bits: 0,
        max_draw_index_value: 0,
        max_draw_indirect_count: 0,
        max_sampler_lod_bias: 0.0, //MAX_TEXTURE_LOD_BIAS
        max_sampler_anisotropy: 0.0,
        max_viewports: 1,
        max_viewport_dimensions: get_u32_pair(gl, gl::MAX_VIEWPORT_DIMS),
        viewport_bounds_range: get_f32_range(gl, gl::VIEWPORT_BOUNDS_RANGE),
        viewport_sub_pixel_bits: get_u32(gl, gl::VIEWPORT_SUBPIXEL_BITS),
        min_memory_map_alignment: 0,
        min_texel_buffer_offset_alignment: 0,
        min_uniform_buffer_offset_alignment: 0,
        min_storage_buffer_offset_alignment: 0,
        texel_offset_bounds: get_isize(gl, gl::MIN_PROGRAM_TEXEL_OFFSET)..=get_isize(gl, gl::MAX_PROGRAM_TEXEL_OFFSET), //TODO
        texel_gather_offset_bounds: (), //(MIN|MAX)_PROGRAM_TEXTURE_GATHER_OFFSET (textureGather)
        interpolation_offset_bounds: get_f32(gl, gl::MIN_FRAGMENT_INTERPOLATION_OFFSET)..=get_f32(gl, gl::MAX_FRAGMENT_INTERPOLATION_OFFSET),
        sub_pixel_interpolation_offset_bits: get_u32(gl, gl::FRAGMENT_INTERPOLATION_OFFSET_BITS),
        max_framebuffer_dimensions: [
            get_u32(gl, gl::MAX_FRAMEBUFFER_WIDTH) as _,
            get_u32(gl, gl::MAX_FRAMEBUFFER_HEIGHT) as _,
            get_u32(gl, gl::MAX_FRAMEBUFFER_LAYERS) as _,
        ],
        framebuffer_color_sample_counts: 0,
        framebuffer_depth_sample_counts: 0,
        framebuffer_stencil_sample_counts: 0,
        framebuffer_none_sample_counts: 0,
        max_subpass_color_attachments: 0, //MAX_COLOR_ATTACHMENTS
        sampled_image_color_sample_counts: 0,
        sampled_image_integer_sample_counts: ((get_u32(gl, gl::MAX_INTEGER_SAMPLES) + 1).next_power_of_two() >> 1) - 1,
        sampled_image_depth_sample_counts: 0,
        sampled_image_stencil_sample_counts: 0,
        storage_image_sample_counts: 0,
        max_sample_mask_words: get_u32(gl, gl::MAX_SAMPLE_MASK_WORDS),
        all_compute_and_graphics_support_timestamps: false,
        timestamp_precision: 0.0,
        max_clip_distances: get_u32(gl, gl::MAX_CLIP_DISTANCES),
        max_cull_distances: 0,
        max_clip_cull_distances_total: 0,
        discrete_queue_priorities: 0,
        point_size_range: get_f32_range(gl, gl::POINT_SIZE_RANGE),
        line_width_range: (),
        point_size_granularity: get_f32(gl, gl::POINT_SIZE_GRANULARITY),
        line_width_granularity: 0.0,
        is_strict_lines: true,
        is_standard_sampler_locations: false,
        optimal_buffer_copy_offset_alignment: 0,
        optimal_buffer_copy_pitch_alignment: 0,
        non_coherent_atom_size: 0,
    };

    if info.is_supported(&[
        Core(4,0),
        Ext("GL_ARB_tessellation_shader"),
    ]) {
        limits.max_patch_size = get_u32(gl, gl::MAX_PATCH_VERTICES) as _;
    }
    if info.is_supported(&[Core(4,1)]) { // TODO: extension
        limits.max_viewports = get_u32(gl, gl::MAX_VIEWPORTS);
    }

    if false && info.is_supported(&[ //TODO: enable when compute is implemented
        Core(4, 3),
        Ext("GL_ARB_compute_shader"),
    ]) {
        let mut values = [0 as gl::types::GLint; 2];
        for (i, (count, size)) in limits.max_compute_group_count
            .iter_mut()
            .zip(limits.max_compute_group_size.iter_mut())
            .enumerate()
        {
            *count = get_indexed_u32(gl, gl::MAX_COMPUTE_WORK_GROUP_COUNT, i as _);
            *size = get_indexed_u32(gl, gl::MAX_COMPUTE_WORK_GROUP_SIZE, i as _);
        }
        limits.max_compute_group_invocations = get_u32(gl, gl::MAX_COMPUTE_WORK_GROUP_INVOCATIONS);
    }

    let mut features = Features::empty();
    let mut legacy = LegacyFeatures::empty();

    if info.is_supported(&[
        Core(4, 6),
        Ext("GL_ARB_texture_filter_anisotropic"),
        Ext("GL_EXT_texture_filter_anisotropic"),
    ]) {
        features |= Features::SAMPLER_ANISOTROPY;
    }
    if info.is_supported(&[
        Core(4, 2),
    ]) {
        legacy |= LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER;
    }
    if info.is_supported(&[
        Core(3, 3),
        Es(3, 0),
        Ext("GL_ARB_instanced_arrays"),
    ]) {
        features |= Features::INSTANCE_RATE;
    }

    if info.is_supported(&[Core(4, 3), Es(3, 1)]) { // TODO: extension
        legacy |= LegacyFeatures::INDIRECT_EXECUTION;
    }
    if info.is_supported(&[
        Core(3, 1),
        Es(3, 0),
        Ext("GL_ARB_draw_instanced"),
    ]) {
        legacy |= LegacyFeatures::DRAW_INSTANCED;
    }
    if info.is_supported(&[
        Core(4, 2),
        Ext("GL_ARB_base_instance"),
    ]) {
        legacy |= LegacyFeatures::DRAW_INSTANCED_BASE;
    }
    if info.is_supported(&[Core(3, 2)]) { // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_BASE;
    }
    if info.is_supported(&[Core(3, 1), Es(3, 0)]) { // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_INSTANCED;
    }
    if info.is_supported(&[Core(3, 2)]) { // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE_VERTEX;
    }
    if info.is_supported(&[Core(4, 2)]) { // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE;
    }
    if info.is_supported(&[
        Core(3, 2),
        Es(3, 2),
        Ext("GL_ARB_draw_elements_base_vertex"),
    ]) {
        legacy |= LegacyFeatures::VERTEX_BASE;
    }
    if info.is_supported(&[
        Core(3, 2),
        Ext("GL_ARB_framebuffer_sRGB"),
    ]) {
        legacy |= LegacyFeatures::SRGB_COLOR;
    }
    if info.is_supported(&[
        Core(3, 1),
        Es(3, 0),
        Ext("GL_ARB_uniform_buffer_object"),
    ]) {
        legacy |= LegacyFeatures::CONSTANT_BUFFER;
    }
    if info.is_supported(&[Core(4, 0)]) { // TODO: extension
        legacy |= LegacyFeatures::UNORDERED_ACCESS_VIEW;
    }
    if info.is_supported(&[
        Core(3, 1),
        Es(3, 0),
        Ext("GL_ARB_copy_buffer"),
        Ext("GL_NV_copy_buffer"),
    ]) {
        legacy |= LegacyFeatures::COPY_BUFFER;
    }
    if info.is_supported(&[
        Core(3, 3),
        Es(3, 0),
        Ext("GL_ARB_sampler_objects"),
    ]) {
        legacy |= LegacyFeatures::SAMPLER_OBJECTS;
    }
    if info.is_supported(&[Core(3, 3)]) { // TODO: extension
        legacy |= LegacyFeatures::SAMPLER_LOD_BIAS;
    }
    if info.is_supported(&[Core(3, 3)]) { // TODO: extension
        legacy |= LegacyFeatures::SAMPLER_BORDER_COLOR;
    }

    let private = PrivateCaps {
        vertex_array:                       info.is_supported(&[Core(3,0),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_vertex_array_object")]),
        framebuffer:                        info.is_supported(&[Core(3,0),
                                                                Es  (2,0),
                                                                Ext ("GL_ARB_framebuffer_object")]),
        framebuffer_texture:                info.is_supported(&[Core(3,0)]), //TODO: double check
        buffer_role_change:                 !info.version.is_embedded,
        image_storage:                      info.is_supported(&[Core(3,2),
                                                                Ext ("GL_ARB_texture_storage")]),
        buffer_storage:                     info.is_supported(&[Core(4,4),
                                                                Ext ("GL_ARB_buffer_storage")]),
        clear_buffer:                       info.is_supported(&[Core(3,0),
                                                                Es  (3,0)]),
        program_interface:                  info.is_supported(&[Core(4,3),
                                                                Ext ("GL_ARB_program_interface_query")]),
        frag_data_location:                 !info.version.is_embedded,
        sync:                               info.is_supported(&[Core(3,2),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_sync")]),
        map:                                !info.version.is_embedded, //TODO: OES extension
        sampler_anisotropy_ext:             !info.is_supported(&[Core(4,6),
                                                                Ext ("GL_ARB_texture_filter_anisotropic")]) &&
                                            info.is_supported(&[Ext ("GL_EXT_texture_filter_anisotropic")]),
    };

    (info, features, legacy, limits, private)
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn test_version_parse() {
        assert_eq!(Version::parse("1"), Err("1"));
        assert_eq!(Version::parse("1."), Err("1."));
        assert_eq!(Version::parse("1 h3l1o. W0rld"), Err("1 h3l1o. W0rld"));
        assert_eq!(Version::parse("1. h3l1o. W0rld"), Err("1. h3l1o. W0rld"));
        assert_eq!(Version::parse("1.2.3"), Ok(Version::new(1, 2, Some(3), "")));
        assert_eq!(Version::parse("1.2"), Ok(Version::new(1, 2, None, "")));
        assert_eq!(Version::parse("1.2 h3l1o. W0rld"), Ok(Version::new(1, 2, None, "h3l1o. W0rld")));
        assert_eq!(Version::parse("1.2.h3l1o. W0rld"), Ok(Version::new(1, 2, None, "W0rld")));
        assert_eq!(Version::parse("1.2. h3l1o. W0rld"), Ok(Version::new(1, 2, None, "h3l1o. W0rld")));
        assert_eq!(Version::parse("1.2.3.h3l1o. W0rld"), Ok(Version::new(1, 2, Some(3), "W0rld")));
        assert_eq!(Version::parse("1.2.3 h3l1o. W0rld"), Ok(Version::new(1, 2, Some(3), "h3l1o. W0rld")));
        assert_eq!(Version::parse("OpenGL ES 3.1"), Ok(Version::new_embedded(3, 1, "")));
        assert_eq!(Version::parse("OpenGL ES 2.0 Google Nexus"), Ok(Version::new_embedded(2, 0, "Google Nexus")));
        assert_eq!(Version::parse("GLSL ES 1.1"), Ok(Version::new_embedded(1, 1, "")));
    }
}
