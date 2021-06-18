use crate::{Error, GlContainer, MAX_COLOR_ATTACHMENTS};
use glow::HasContext;
use hal::{DynamicStates, Features, Limits, PerformanceCaveats, PhysicalDeviceProperties};
use std::{collections::HashSet, fmt, str};

/// A version number for a specific component of an OpenGL implementation
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub is_embedded: bool,
    pub revision: Option<u32>,
    pub vendor_info: String,
}

impl Version {
    /// Create a new OpenGL version number
    pub fn new(major: u32, minor: u32, revision: Option<u32>, vendor_info: String) -> Self {
        Version {
            major: major,
            minor: minor,
            is_embedded: false,
            revision: revision,
            vendor_info,
        }
    }
    /// Create a new OpenGL ES version number
    pub fn new_embedded(major: u32, minor: u32, vendor_info: String) -> Self {
        Version {
            major,
            minor,
            is_embedded: true,
            revision: None,
            vendor_info,
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
    /// # Notes
    /// `WebGL 2` version returned as `OpenGL ES 3.0`
    pub fn parse(mut src: &str) -> Result<Version, &str> {
        let webgl_sig = "WebGL ";
        // According to the WebGL specification
        // VERSION	WebGL<space>1.0<space><vendor-specific information>
        // SHADING_LANGUAGE_VERSION	WebGL<space>GLSL<space>ES<space>1.0<space><vendor-specific information>
        let is_webgl = src.starts_with(webgl_sig);
        let is_es = if is_webgl {
            let pos = src.rfind(webgl_sig).unwrap_or(0);
            src = &src[pos + webgl_sig.len()..];
            true
        } else {
            let es_sig = " ES ";
            match src.rfind(es_sig) {
                Some(pos) => {
                    src = &src[pos + es_sig.len()..];
                    true
                }
                None => false,
            }
        };

        let glsl_es_sig = "GLSL ES ";
        let is_glsl = match src.find(glsl_es_sig) {
            Some(pos) => {
                src = &src[pos + glsl_es_sig.len()..];
                true
            }
            None => false,
        };

        let (version, vendor_info) = match src.find(' ') {
            Some(i) => (&src[..i], src[i + 1..].to_string()),
            None => (src, String::new()),
        };

        // TODO: make this even more lenient so that we can also accept
        // `<major> "." <minor> [<???>]`
        let mut it = version.split('.');
        let major = it.next().and_then(|s| s.parse().ok());
        let minor = it.next().and_then(|s| {
            let trimmed = if s.starts_with('0') {
                "0"
            } else {
                s.trim_end_matches('0')
            };
            trimmed.parse().ok()
        });
        let revision = if is_webgl {
            None
        } else {
            it.next().and_then(|s| s.parse().ok())
        };

        match (major, minor, revision) {
            (Some(major), Some(minor), revision) => Ok(Version {
                // Return WebGL 2.0 version as OpenGL ES 3.0
                major: if is_webgl && !is_glsl {
                    major + 1
                } else {
                    major
                },
                minor,
                is_embedded: is_es,
                revision,
                vendor_info,
            }),
            (_, _, _) => Err(src),
        }
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (
            self.major,
            self.minor,
            self.revision,
            self.vendor_info.as_str(),
        ) {
            (major, minor, Some(revision), "") => write!(f, "{}.{}.{}", major, minor, revision),
            (major, minor, None, "") => write!(f, "{}.{}", major, minor),
            (major, minor, Some(revision), vendor_info) => {
                write!(f, "{}.{}.{}, {}", major, minor, revision, vendor_info)
            }
            (major, minor, None, vendor_info) => write!(f, "{}.{}, {}", major, minor, vendor_info),
        }
    }
}

fn get_string(gl: &GlContainer, name: u32) -> Result<String, Error> {
    let value = unsafe { gl.get_parameter_string(name) };
    let err = Error::from_error_code(unsafe { gl.get_error() });
    if err != Error::NoError {
        Err(err)
    } else {
        Ok(value)
    }
}
fn get_usize(gl: &GlContainer, name: u32) -> Result<usize, Error> {
    let value = unsafe { gl.get_parameter_i32(name) };
    let err = Error::from_error_code(unsafe { gl.get_error() });
    if err != Error::NoError {
        Err(err)
    } else {
        Ok(value as usize)
    }
}
fn get_u64(gl: &GlContainer, name: u32) -> Result<u64, Error> {
    let value = unsafe { gl.get_parameter_i32(name) };
    let err = Error::from_error_code(unsafe { gl.get_error() });
    if err != Error::NoError {
        Err(err)
    } else {
        Ok(value as u64)
    }
}

/// A unique platform identifier that does not change between releases
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PlatformName {
    /// The company responsible for the OpenGL implementation
    pub vendor: String,
    /// The name of the renderer
    pub renderer: String,
}

impl PlatformName {
    fn get(gl: &GlContainer) -> Self {
        PlatformName {
            vendor: get_string(gl, glow::VENDOR).unwrap_or_default(),
            renderer: get_string(gl, glow::RENDERER).unwrap_or_default(),
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
    /// If true, then buffers used as ELEMENT_ARRAY_BUFFER may be created / initialized / used as
    /// other targets, if false they must not be mixed with other targets.
    pub index_buffer_role_change: bool,
    pub buffer_storage: bool,
    pub image_storage: bool,
    pub clear_buffer: bool,
    pub program_interface: bool,
    pub frag_data_location: bool,
    pub sync: bool,
    /// Whether to emulate memory mapping (`glMapBuffer`/`glMapBufferRange`)
    /// when it is not available:
    /// - In OpenGL ES 2 it may be available behind optional extensions
    /// - In WebGL 1 and WebGL 2 it is never available
    /// - In OpenGL, currently required to get copies from/to buffers working:
    /// https://github.com/gfx-rs/gfx/issues/3453
    pub emulate_map: bool,
    /// Whether f64 precision is supported for depth ranges
    pub depth_range_f64_precision: bool,
    /// Whether draw buffers are supported
    pub draw_buffers: bool,
    /// Whether separate color masks per output buffer are supported.
    pub per_slot_color_mask: bool,
    /// Reading from textures into CPU memory is supported.
    pub get_tex_image: bool,
    /// Inserting memory barriers.
    pub memory_barrier: bool,
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
    pub extensions: HashSet<String>,
}

bitflags::bitflags! {
    /// Flags for features that are required for Vulkan but may not
    /// be supported by legacy backends (GL/DX11).
    pub struct LegacyFeatures: u32 {
        /// Support indirect drawing and dispatching.
        const INDIRECT_EXECUTION = 0x00000001;
        /// Support instanced drawing.
        const DRAW_INSTANCED = 0x00000002;
        /// Support offsets for instanced drawing with base instance.
        const DRAW_INSTANCED_BASE = 0x00000004;
        /// Support indexed drawing with base vertex.
        const DRAW_INDEXED_BASE = 0x00000008;
        /// Support indexed, instanced drawing.
        const DRAW_INDEXED_INSTANCED = 0x00000010;
        /// Support indexed, instanced drawing with base vertex only.
        const DRAW_INDEXED_INSTANCED_BASE_VERTEX = 0x00000020;
        /// Support base vertex offset for indexed drawing.
        const VERTEX_BASE = 0x00000080;
        /// Support sRGB textures and rendertargets.
        const SRGB_COLOR = 0x00000100;
        /// Support constant buffers.
        const CONSTANT_BUFFER = 0x00000200;
        /// Support unordered-access views.
        const UNORDERED_ACCESS_VIEW = 0x00000400;
        /// Support accelerated buffer copy.
        const COPY_BUFFER = 0x00000800;
        /// Support separation of textures and samplers.
        const SAMPLER_OBJECTS = 0x00001000;
        /// Support explicit layouts in shader.
        const EXPLICIT_LAYOUTS_IN_SHADER = 0x00002000;
        /// Support instanced input rate on attribute binding.
        const INSTANCED_ATTRIBUTE_BINDING = 0x00004000;
    }
}

#[derive(Copy, Clone)]
pub enum Requirement<'a> {
    Core(u32, u32),
    Es(u32, u32),
    Ext(&'a str),
}

impl Info {
    fn get(gl: &GlContainer) -> Info {
        let platform_name = PlatformName::get(gl);
        let raw_version = get_string(gl, glow::VERSION).unwrap_or_default();
        let version = Version::parse(&raw_version).unwrap();
        let shading_language = {
            let raw_shader_version =
                get_string(gl, glow::SHADING_LANGUAGE_VERSION).unwrap_or_default();
            Version::parse(&raw_shader_version).unwrap()
        };

        // TODO: Use separate path for WebGL extensions in `glow` somehow
        // Perhaps automatic fallback for NUM_EXTENSIONS to EXTENSIONS on native
        let extensions = if crate::is_webgl() {
            HashSet::new()
        } else if (version >= Version::new(3, 0, None, String::from("")))
            || (version >= Version::new_embedded(3, 0, String::from("")))
        {
            let num_exts = get_usize(gl, glow::NUM_EXTENSIONS).unwrap();
            (0..num_exts)
                .map(|i| unsafe { gl.get_parameter_indexed_string(glow::EXTENSIONS, i as u32) })
                .collect()
        } else {
            // Fallback
            get_string(gl, glow::EXTENSIONS)
                .unwrap_or_else(|_| String::from(""))
                .split(' ')
                .map(|s| s.to_string())
                .collect()
        };

        Info {
            platform_name,
            version,
            shading_language,
            extensions,
        }
    }

    pub fn is_version_supported(&self, major: u32, minor: u32) -> bool {
        !self.version.is_embedded
            && self.version >= Version::new(major, minor, None, String::from(""))
    }

    pub fn is_embedded_version_supported(&self, major: u32, minor: u32) -> bool {
        self.version.is_embedded
            && self.version >= Version::new_embedded(major, minor, String::from(""))
    }

    /// Returns `true` if the implementation supports the extension
    pub fn is_extension_supported(&self, s: &str) -> bool {
        self.extensions.contains(s)
    }

    pub fn is_version_or_extension_supported(&self, major: u32, minor: u32, ext: &str) -> bool {
        self.is_version_supported(major, minor) || self.is_extension_supported(ext)
    }

    pub fn is_any_extension_supported(&self, exts: &[String]) -> bool {
        exts.iter().any(|e| self.extensions.contains(e))
    }

    pub fn is_supported(&self, requirements: &[Requirement]) -> bool {
        use self::Requirement::*;
        requirements.iter().any(|r| match *r {
            Core(major, minor) => self.is_version_supported(major, minor),
            Es(major, minor) => self.is_embedded_version_supported(major, minor),
            Ext(extension) => self.is_extension_supported(extension),
        })
    }
}

/// This structure checks whether a given image format is whitelisted to be used
/// or not in the backend.
///
/// Unlike with Vulkan, OpenGL gives its users no easy way to query for valid
/// image formats. Instead, it relies heavily on semantics, which are laid out
/// in its specifications and function documentations.
///
/// This structure is intended to condense all of the supported formats into a
/// single queryable location, acquired once when the adapter is created.
#[derive(Debug)]
pub enum TextureFormatFilter {
    /// This filter names a set of allowed combinations. All combinations not
    /// explicitly whitelisted will be reported as not available by the check
    /// function.
    Whitelist { whitelist: HashSet<(u32, u32, u32)> },
    /// This filter is permissive, i.e. it reports all combinations as
    /// available. Currently intended for use in the core profile.
    Permissive,
}
impl TextureFormatFilter {
    /// This is the fixed, spec-defined list of all triplets in the form
    /// `(Internal Format, Format, Type)` that are supported by OpenGL ES 3 and
    /// WebGL for image creation via the `glTexImage` family of functions.
    ///
    /// Combinations of these parameters other than the ones in this table are
    /// not supported by the core specifications. Though that does not mean they
    /// can't be used or can't work under any circumstance. What it does mean is
    /// that using anything outside this list is relying on permissive drivers
    /// to not trigger undefined behavior.
    ///
    /// This is a one to one copy of the tables provided to us in
    /// [the documentation of `glTexImage2D`], which is the same table as the
    /// for `glTexImage3D`. The contents of the table are copied pretty much
    /// verbatim in order to facilitate maintenance.
    ///
    /// [the documentation of `glTexImage2D`]: https://www.khronos.org/registry/OpenGL-Refpages/es3/html/glTexImage2D.xhtml
    const ES3_TABLE: &'static [(u32, u32, u32)] = &[
        /* Taken from Table 1. Unsized Internal Formats. */
        (glow::RGB, glow::RGB, glow::UNSIGNED_BYTE),
        (glow::RGB, glow::RGB, glow::UNSIGNED_SHORT_5_6_5),
        (glow::RGBA, glow::RGBA, glow::UNSIGNED_BYTE),
        (glow::RGBA, glow::RGBA, glow::UNSIGNED_SHORT_4_4_4_4),
        (glow::RGBA, glow::RGBA, glow::UNSIGNED_SHORT_5_5_5_1),
        (
            glow::LUMINANCE_ALPHA,
            glow::LUMINANCE_ALPHA,
            glow::UNSIGNED_BYTE,
        ),
        (glow::LUMINANCE, glow::LUMINANCE, glow::UNSIGNED_BYTE),
        (glow::ALPHA, glow::ALPHA, glow::UNSIGNED_BYTE),
        /* Taken from Table 2. Sized Internal Formats. */
        (glow::R8, glow::RED, glow::UNSIGNED_BYTE),
        (glow::R8_SNORM, glow::RED, glow::BYTE),
        (glow::R16F, glow::RED, glow::HALF_FLOAT),
        (glow::R16F, glow::RED, glow::FLOAT),
        (glow::R32F, glow::RED, glow::FLOAT),
        (glow::R8UI, glow::RED_INTEGER, glow::UNSIGNED_BYTE),
        (glow::R8I, glow::RED_INTEGER, glow::BYTE),
        (glow::R16UI, glow::RED_INTEGER, glow::UNSIGNED_SHORT),
        (glow::R16I, glow::RED_INTEGER, glow::SHORT),
        (glow::R32UI, glow::RED_INTEGER, glow::UNSIGNED_INT),
        (glow::R32I, glow::RED_INTEGER, glow::INT),
        (glow::RG8, glow::RG, glow::UNSIGNED_BYTE),
        (glow::RG8_SNORM, glow::RG, glow::BYTE),
        (glow::RG16F, glow::RG, glow::HALF_FLOAT),
        (glow::RG16F, glow::RG, glow::FLOAT),
        (glow::RG32F, glow::RG, glow::FLOAT),
        (glow::RG8UI, glow::RG_INTEGER, glow::UNSIGNED_BYTE),
        (glow::RG8I, glow::RG_INTEGER, glow::BYTE),
        (glow::RG16UI, glow::RG_INTEGER, glow::UNSIGNED_SHORT),
        (glow::RG16I, glow::RG_INTEGER, glow::SHORT),
        (glow::RG32UI, glow::RG_INTEGER, glow::UNSIGNED_INT),
        (glow::RG32I, glow::RG_INTEGER, glow::INT),
        (glow::RGB8, glow::RGB, glow::UNSIGNED_BYTE),
        (glow::SRGB8, glow::RGB, glow::UNSIGNED_BYTE),
        (glow::RGB565, glow::RGB, glow::UNSIGNED_BYTE),
        (glow::RGB565, glow::RGB, glow::UNSIGNED_SHORT_5_6_5),
        (glow::RGB8_SNORM, glow::RGB, glow::BYTE),
        (
            glow::R11F_G11F_B10F,
            glow::RGB,
            glow::UNSIGNED_INT_10F_11F_11F_REV,
        ),
        (glow::R11F_G11F_B10F, glow::RGB, glow::HALF_FLOAT),
        (glow::R11F_G11F_B10F, glow::RGB, glow::FLOAT),
        (glow::RGB9_E5, glow::RGB, glow::UNSIGNED_INT_5_9_9_9_REV),
        (glow::RGB9_E5, glow::RGB, glow::HALF_FLOAT),
        (glow::RGB9_E5, glow::RGB, glow::FLOAT),
        (glow::RGB16F, glow::RGB, glow::HALF_FLOAT),
        (glow::RGB16F, glow::RGB, glow::FLOAT),
        (glow::RGB32F, glow::RGB, glow::FLOAT),
        (glow::RGB8UI, glow::RGB_INTEGER, glow::UNSIGNED_BYTE),
        (glow::RGB8I, glow::RGB_INTEGER, glow::BYTE),
        (glow::RGB16UI, glow::RGB_INTEGER, glow::UNSIGNED_SHORT),
        (glow::RGB16I, glow::RGB_INTEGER, glow::SHORT),
        (glow::RGB32UI, glow::RGB_INTEGER, glow::UNSIGNED_INT),
        (glow::RGB32I, glow::RGB_INTEGER, glow::INT),
        (glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE),
        (glow::SRGB8_ALPHA8, glow::RGBA, glow::UNSIGNED_BYTE),
        (glow::RGBA8_SNORM, glow::RGBA, glow::BYTE),
        (glow::RGB5_A1, glow::RGBA, glow::UNSIGNED_BYTE),
        (glow::RGB5_A1, glow::RGBA, glow::UNSIGNED_SHORT_5_5_5_1),
        (glow::RGB5_A1, glow::RGBA, glow::UNSIGNED_INT_2_10_10_10_REV),
        (glow::RGBA4, glow::RGBA, glow::UNSIGNED_BYTE),
        (glow::RGBA4, glow::RGBA, glow::UNSIGNED_SHORT_4_4_4_4),
        (
            glow::RGB10_A2,
            glow::RGBA,
            glow::UNSIGNED_INT_2_10_10_10_REV,
        ),
        (glow::RGBA16F, glow::RGBA, glow::HALF_FLOAT),
        (glow::RGBA16F, glow::RGBA, glow::FLOAT),
        (glow::RGBA32F, glow::RGBA, glow::FLOAT),
        (glow::RGBA8UI, glow::RGBA_INTEGER, glow::UNSIGNED_BYTE),
        (glow::RGBA8I, glow::RGBA_INTEGER, glow::BYTE),
        (
            glow::RGB10_A2UI,
            glow::RGBA_INTEGER,
            glow::UNSIGNED_INT_2_10_10_10_REV,
        ),
        (glow::RGBA16UI, glow::RGBA_INTEGER, glow::UNSIGNED_SHORT),
        (glow::RGBA16I, glow::RGBA_INTEGER, glow::SHORT),
        (glow::RGBA32I, glow::RGBA_INTEGER, glow::INT),
        (glow::RGBA32UI, glow::RGBA_INTEGER, glow::UNSIGNED_INT),
        (
            glow::DEPTH_COMPONENT16,
            glow::DEPTH_COMPONENT,
            glow::UNSIGNED_SHORT,
        ),
        (
            glow::DEPTH_COMPONENT16,
            glow::DEPTH_COMPONENT,
            glow::UNSIGNED_INT,
        ),
        (
            glow::DEPTH_COMPONENT24,
            glow::DEPTH_COMPONENT,
            glow::UNSIGNED_INT,
        ),
        (glow::DEPTH_COMPONENT32F, glow::DEPTH_COMPONENT, glow::FLOAT),
        (
            glow::DEPTH24_STENCIL8,
            glow::DEPTH_STENCIL,
            glow::UNSIGNED_INT_24_8,
        ),
        (
            glow::DEPTH32F_STENCIL8,
            glow::DEPTH_STENCIL,
            glow::FLOAT_32_UNSIGNED_INT_24_8_REV,
        ),
        (
            glow::STENCIL_INDEX8,
            glow::STENCIL_INDEX,
            glow::UNSIGNED_BYTE,
        ),
    ];

    /// Fixed list of format and type combinations supported by both OpenGL ES 1
    /// and OpenGL ES 2. These values stayed the same between these two
    /// versions, so we can just lump them together.
    ///
    /// This is a one to one copy of the tables provided to us in
    /// [the documentation of `glTexImage2D`], which is the same table as the
    /// for `glTexImage3D`. The contents of the table are copied pretty much
    /// verbatim in order to facilitate maintenance.
    ///
    /// [the documentation of `glTexImage2D`]: https://khronos.org/registry/OpenGL-Refpages/es1.1/xhtml/
    const ES1_ES2_TABLE: &'static [(u32, u32, u32)] = &[
        (glow::ALPHA, glow::ALPHA, glow::UNSIGNED_BYTE),
        (glow::RGB, glow::RGB, glow::UNSIGNED_BYTE),
        (glow::RGB, glow::RGB, glow::UNSIGNED_SHORT_5_6_5),
        (glow::RGBA, glow::RGBA, glow::UNSIGNED_BYTE),
        (glow::RGBA, glow::RGBA, glow::UNSIGNED_SHORT_4_4_4_4),
        (glow::RGBA, glow::RGBA, glow::UNSIGNED_SHORT_5_5_5_1),
        (glow::LUMINANCE, glow::LUMINANCE, glow::UNSIGNED_BYTE),
        (
            glow::LUMINANCE_ALPHA,
            glow::LUMINANCE_ALPHA,
            glow::UNSIGNED_BYTE,
        ),
    ];

    /// Creates a new filter for OpenGL ES 3.
    fn new_es3() -> Self {
        Self::Whitelist {
            whitelist: {
                let mut whitelist = HashSet::<(u32, u32, u32)>::default();
                whitelist.extend(Self::ES3_TABLE);

                whitelist
            },
        }
    }

    /// Creates a new filter for OpenGL ES 2 and OpenGL ES 1.
    fn new_es1_es2() -> Self {
        Self::Whitelist {
            whitelist: {
                let mut whitelist = HashSet::<(u32, u32, u32)>::default();
                whitelist.extend(Self::ES1_ES2_TABLE);

                whitelist
            },
        }
    }

    /// Creates a new, permissive filter.
    fn new_permissive() -> Self {
        Self::Permissive
    }

    /// This function checks whether a given format description is allowed by
    /// this filter.
    pub fn check(&self, internal_format: u32, format: u32, type_: u32) -> bool {
        match self {
            Self::Whitelist { whitelist } => whitelist.contains(&(internal_format, format, type_)),
            Self::Permissive => true,
        }
    }
}

/// Load the information pertaining to the driver and the corresponding device
/// capabilities.
pub(crate) fn query_all(
    gl: &GlContainer,
) -> (
    Info,
    Features,
    LegacyFeatures,
    PhysicalDeviceProperties,
    PrivateCaps,
    TextureFormatFilter,
) {
    use self::Requirement::*;
    let info = Info::get(gl);
    let max_texture_size = get_usize(gl, glow::MAX_TEXTURE_SIZE).unwrap_or(64) as u32;
    let max_samples = get_usize(gl, glow::MAX_SAMPLES).unwrap_or(8);
    let max_samples_mask = (max_samples * 2 - 1) as u8;
    let max_texel_elements = if crate::is_webgl() {
        0
    } else {
        get_usize(gl, glow::MAX_TEXTURE_BUFFER_SIZE).unwrap_or(0)
    };
    let min_storage_buffer_offset_alignment = if crate::is_webgl() {
        256
    } else {
        get_u64(gl, glow::SHADER_STORAGE_BUFFER_OFFSET_ALIGNMENT).unwrap_or(256)
    };

    let mut limits = Limits {
        max_image_1d_size: max_texture_size,
        max_image_2d_size: max_texture_size,
        max_image_3d_size: max_texture_size,
        max_image_cube_size: max_texture_size,
        max_image_array_layers: get_usize(gl, glow::MAX_ARRAY_TEXTURE_LAYERS).unwrap_or(1) as u16,
        max_texel_elements,
        max_viewports: 1,
        optimal_buffer_copy_offset_alignment: 1,
        optimal_buffer_copy_pitch_alignment: 1,
        min_texel_buffer_offset_alignment: 1,
        min_uniform_buffer_offset_alignment: get_u64(gl, glow::UNIFORM_BUFFER_OFFSET_ALIGNMENT)
            .unwrap_or(1024),
        min_storage_buffer_offset_alignment,
        framebuffer_color_sample_counts: max_samples_mask,
        non_coherent_atom_size: 1,
        max_color_attachments: get_usize(gl, glow::MAX_COLOR_ATTACHMENTS)
            .unwrap_or(1)
            .min(MAX_COLOR_ATTACHMENTS),
        max_memory_allocation_count: 4096,
        ..Limits::default()
    };

    if info.is_supported(&[Core(4, 0), Ext("GL_ARB_tessellation_shader")]) {
        limits.max_patch_size = get_usize(gl, glow::MAX_PATCH_VERTICES).unwrap_or(0) as _;
    }
    if info.is_supported(&[Core(4, 1)]) {
        // TODO: extension
        limits.max_viewports = get_usize(gl, glow::MAX_VIEWPORTS).unwrap_or(0);
    }

    //TODO: technically compute is exposed in Es(3, 1), but GLES requires 3.2
    // for any storage buffers. We need to investigate if this requirement
    // can be lowered.
    if info.is_supported(&[Core(4, 3), Es(3, 2), Ext("GL_ARB_compute_shader")]) {
        for (i, (count, size)) in limits
            .max_compute_work_group_count
            .iter_mut()
            .zip(limits.max_compute_work_group_size.iter_mut())
            .enumerate()
        {
            unsafe {
                *count =
                    gl.get_parameter_indexed_i32(glow::MAX_COMPUTE_WORK_GROUP_COUNT, i as _) as u32;
                *size =
                    gl.get_parameter_indexed_i32(glow::MAX_COMPUTE_WORK_GROUP_SIZE, i as _) as u32;
            }
        }
    }

    let mut features = Features::NDC_Y_UP | Features::MUTABLE_COMPARISON_SAMPLER;
    // TODO: Fill out downlevel features correctly.
    let mut downlevel = hal::DownlevelProperties::all_enabled();
    // TODO: Merge downlevel/legacy features?
    let mut legacy = LegacyFeatures::empty();

    if info.is_supported(&[
        Core(4, 6),
        Ext("GL_ARB_texture_filter_anisotropic"),
        Ext("GL_EXT_texture_filter_anisotropic"),
    ]) {
        features |= Features::SAMPLER_ANISOTROPY;
    }
    if info.is_supported(&[Core(4, 2), Es(3, 1)]) {
        legacy |= LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER;
    }
    if info.is_supported(&[Core(3, 3), Es(3, 0), Ext("GL_ARB_instanced_arrays")]) {
        features |= Features::INSTANCE_RATE;
    }
    if info.is_supported(&[Core(3, 3)]) {
        // TODO: extension
        features |= Features::SAMPLER_MIP_LOD_BIAS;
    }
    if info.is_supported(&[Core(2, 1)]) {
        features |= Features::SAMPLER_BORDER_COLOR;
    }
    if info.is_supported(&[Core(4, 4), Ext("ARB_texture_mirror_clamp_to_edge")]) {
        features |= Features::SAMPLER_MIRROR_CLAMP_EDGE;
    }
    if info.is_supported(&[Core(4, 0), Es(3, 2), Ext("GL_EXT_draw_buffers2")]) && !crate::is_webgl()
    {
        features |= Features::INDEPENDENT_BLENDING;
    }
    if info.is_supported(&[
        Es(3, 0),
        Ext("WEBGL_compressed_texture_s3tc"),
        Ext("WEBGL_compressed_texture_s3tc_srgb"),
    ]) {
        features |= Features::FORMAT_BC;
    }

    // TODO
    if false && info.is_supported(&[Core(4, 3), Es(3, 1)]) {
        // TODO: extension
        legacy |= LegacyFeatures::INDIRECT_EXECUTION;
    }
    if info.is_supported(&[Core(3, 1), Es(3, 0), Ext("GL_ARB_draw_instanced")]) {
        legacy |= LegacyFeatures::DRAW_INSTANCED;
    }
    if info.is_supported(&[Core(4, 2), Ext("GL_ARB_base_instance")]) {
        legacy |= LegacyFeatures::DRAW_INSTANCED_BASE;
    }
    if info.is_supported(&[Core(3, 2)]) {
        // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_BASE;
    }
    if info.is_supported(&[Core(3, 1), Es(3, 0)]) {
        // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_INSTANCED;
    }
    if info.is_supported(&[Core(3, 2)]) {
        // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE_VERTEX;
    }
    if info.is_supported(&[
        Core(3, 2),
        Es(3, 2),
        Ext("GL_ARB_draw_elements_base_vertex"),
    ]) {
        legacy |= LegacyFeatures::VERTEX_BASE;
    }
    if info.is_supported(&[
        Core(3, 1),
        Es(3, 0),
        Ext("GL_ARB_framebuffer_sRGB"),
        Ext("GL_EXT_sRGB"),
    ]) {
        legacy |= LegacyFeatures::SRGB_COLOR;
    }
    if info.is_supported(&[Core(3, 1), Es(3, 0), Ext("GL_ARB_uniform_buffer_object")]) {
        legacy |= LegacyFeatures::CONSTANT_BUFFER;
    }
    if info.is_supported(&[Core(4, 0)]) {
        // TODO: extension
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
    if info.is_supported(&[Core(3, 3), Es(3, 0), Ext("GL_ARB_sampler_objects")]) {
        legacy |= LegacyFeatures::SAMPLER_OBJECTS;
    }
    if info.is_supported(&[Core(3, 3), Es(3, 0)]) {
        legacy |= LegacyFeatures::INSTANCED_ATTRIBUTE_BINDING;
    }
    let mut performance_caveats = PerformanceCaveats::empty();
    //TODO: extension
    if !info.is_supported(&[Core(4, 2)]) {
        performance_caveats |= PerformanceCaveats::BASE_VERTEX_INSTANCE_DRAWING;
    }
    let properties = PhysicalDeviceProperties {
        limits,
        performance_caveats,
        dynamic_pipeline_states: DynamicStates::all(),
        ..PhysicalDeviceProperties::default()
    };

    let buffer_storage = info.is_supported(&[
        Core(4, 4),
        Ext("GL_ARB_buffer_storage"),
        Ext("GL_EXT_buffer_storage"),
    ]);
    // See https://github.com/gfx-rs/gfx/issues/3453
    let emulate_map = crate::is_webgl() || !buffer_storage;

    let private = PrivateCaps {
        vertex_array: info.is_supported(&[Core(3, 0), Es(3, 0), Ext("GL_ARB_vertex_array_object")]),
        // TODO && gl.GenVertexArrays.is_loaded(),
        framebuffer: info.is_supported(&[Core(3, 0), Es(2, 0), Ext("GL_ARB_framebuffer_object")]),
        // TODO && gl.GenFramebuffers.is_loaded(),
        framebuffer_texture: info.is_supported(&[Core(3, 0)]), //TODO: double check
        // `WebGL` Note: buffers bound to non ELEMENT_ARRAY_BUFFER targets can not be bound to ELEMENT_ARRAY_BUFFER target
        index_buffer_role_change: info.is_supported(&[Core(2, 0), Es(2, 0)]) && !crate::is_webgl(),
        image_storage: info.is_supported(&[Core(4, 2), Es(3, 0), Ext("GL_ARB_texture_storage")]),
        buffer_storage,
        clear_buffer: info.is_supported(&[Core(3, 0), Es(3, 0)]),
        program_interface: info.is_supported(&[Core(4, 3), Ext("GL_ARB_program_interface_query")]),
        frag_data_location: !info.version.is_embedded,
        sync: info.is_supported(&[Core(3, 2), Es(3, 0), Ext("GL_ARB_sync")]), // TODO
        emulate_map,
        depth_range_f64_precision: !info.version.is_embedded, // TODO
        draw_buffers: info.is_supported(&[Core(2, 0), Es(3, 0)]),
        per_slot_color_mask: info.is_supported(&[Core(3, 0)]),
        get_tex_image: !info.version.is_embedded,
        memory_barrier: info.is_supported(&[Core(4, 2), Es(3, 1)]),
    };

    let filter = if info.is_supported(&[Es(3, 0)]) {
        /* Use the OpenGL ES 3 format filter. */
        TextureFormatFilter::new_es3()
    } else if info.is_supported(&[Es(1, 0)]) {
        /* Use the OpenGL ES 1 and OpenGL ES 2 format filter. */
        TextureFormatFilter::new_es1_es2()
    } else {
        /* We're using the core specification. We can assume all of the
         * combinations are valid, provided the OpenGL enums values are also
         * valid for textures. */
        TextureFormatFilter::new_permissive()
    };

    (info, features, legacy, properties, private, filter)
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
        assert_eq!(
            Version::parse("1.2.3"),
            Ok(Version::new(1, 2, Some(3), String::new()))
        );
        assert_eq!(
            Version::parse("1.2"),
            Ok(Version::new(1, 2, None, String::new()))
        );
        assert_eq!(
            Version::parse("1.2 h3l1o. W0rld"),
            Ok(Version::new(1, 2, None, "h3l1o. W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2.h3l1o. W0rld"),
            Ok(Version::new(1, 2, None, "W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2. h3l1o. W0rld"),
            Ok(Version::new(1, 2, None, "h3l1o. W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2.3.h3l1o. W0rld"),
            Ok(Version::new(1, 2, Some(3), "W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2.3 h3l1o. W0rld"),
            Ok(Version::new(1, 2, Some(3), "h3l1o. W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("OpenGL ES 3.1"),
            Ok(Version::new_embedded(3, 1, String::new()))
        );
        assert_eq!(
            Version::parse("OpenGL ES 2.0 Google Nexus"),
            Ok(Version::new_embedded(2, 0, "Google Nexus".to_string()))
        );
        assert_eq!(
            Version::parse("GLSL ES 1.1"),
            Ok(Version::new_embedded(1, 1, String::new()))
        );
        assert_eq!(
            Version::parse("OpenGL ES GLSL ES 3.20"),
            Ok(Version::new_embedded(3, 2, String::new()))
        );
        assert_eq!(
            // WebGL 2.0 should parse as OpenGL ES 3.0
            Version::parse("WebGL 2.0 (OpenGL ES 3.0 Chromium)"),
            Ok(Version::new_embedded(
                3,
                0,
                "(OpenGL ES 3.0 Chromium)".to_string()
            ))
        );
        assert_eq!(
            Version::parse("WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)"),
            Ok(Version::new_embedded(
                3,
                0,
                "(OpenGL ES GLSL ES 3.0 Chromium)".to_string()
            ))
        );
    }
}
