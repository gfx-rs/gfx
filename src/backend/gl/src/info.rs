use crate::hal::{Features, Limits};
use crate::{Error, GlContainer};
use std::collections::HashSet;
use std::{fmt, str};

use glow::Context;

/// A version number for a specific component of an OpenGL implementation
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    pub is_embedded: bool,
    pub major: u32,
    pub minor: u32,
    pub revision: Option<u32>,
    pub vendor_info: String,
}

impl Version {
    /// Create a new OpenGL version number
    pub fn new(major: u32, minor: u32, revision: Option<u32>, vendor_info: String) -> Self {
        Version {
            is_embedded: false,
            major: major,
            minor: minor,
            revision: revision,
            vendor_info: vendor_info,
        }
    }
    /// Create a new OpenGL ES version number
    pub fn new_embedded(major: u32, minor: u32, vendor_info: String) -> Self {
        Version {
            is_embedded: true,
            major,
            minor,
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
    pub fn parse(mut src: String) -> Result<Version, String> {
        // TODO: Parse version and optional vendor
        let webgl_sig = "WebGL ";
        let is_webgl = src.contains(webgl_sig);
        if is_webgl {
            return Ok(Version {
                is_embedded: true,
                major: 2,
                minor: 0,
                revision: None,
                vendor_info: "".to_string(),
            });
        }

        let es_sig = " ES ";
        let is_es = match src.rfind(es_sig) {
            Some(pos) => {
                src = src[pos + es_sig.len()..].to_string();
                true
            }
            None => false,
        };
        let (version, vendor_info) = match src.find(' ') {
            Some(i) => (src[..i].to_string(), src[i + 1..].to_string()),
            None => (src.to_string(), String::from("")),
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
                major,
                minor,
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
            vendor: get_string(gl, glow::VENDOR).unwrap(),
            renderer: get_string(gl, glow::RENDERER).unwrap(),
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
    /// Whether to emulate map memory
    pub emulate_map: bool,
    /// Indicates if we only have support via the EXT.
    pub sampler_anisotropy_ext: bool,
    /// Whether f64 precision is supported for depth ranges
    pub depth_range_f64_precision: bool,
    /// Whether draw buffers are supported
    pub draw_buffers: bool,
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

bitflags! {
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
        /// Support indexed, instanced drawing with base vertex and instance.
        const DRAW_INDEXED_INSTANCED_BASE = 0x00000040;
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
        /// Support setting border texel colors.
        const SAMPLER_BORDER_COLOR = 0x00002000;
        /// Support explicit layouts in shader.
        const EXPLICIT_LAYOUTS_IN_SHADER = 0x00004000;
        /// Support instanced input rate on attribute binding.
        const INSTANCED_ATTRIBUTE_BINDING = 0x00008000;
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
        let version =
            Version::parse(get_string(gl, glow::VERSION).unwrap_or_else(|_| String::from("")))
                .unwrap();
        #[cfg(not(target_arch = "wasm32"))]
        let shading_language = Version::parse(
            get_string(gl, glow::SHADING_LANGUAGE_VERSION).unwrap_or_else(|_| String::from("")),
        )
        .unwrap();
        #[cfg(target_arch = "wasm32")]
        let shading_language = Version::new_embedded(3, 0, String::from(""));
        // TODO: Use separate path for WebGL extensions in `glow` somehow
        // Perhaps automatic fallback for NUM_EXTENSIONS to EXTENSIONS on native
        #[cfg(target_arch = "wasm32")]
        let extensions = HashSet::new();
        #[cfg(not(target_arch = "wasm32"))]
        let extensions = if version >= Version::new(3, 0, None, String::from("")) {
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
            && self.version >= Version::new(major, minor, None, String::from(""))
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

/// Load the information pertaining to the driver and the corresponding device
/// capabilities.
pub(crate) fn query_all(gl: &GlContainer) -> (Info, Features, LegacyFeatures, Limits, PrivateCaps) {
    use self::Requirement::*;
    let info = Info::get(gl);
    let max_texture_size = get_usize(gl, glow::MAX_TEXTURE_SIZE).unwrap_or(64) as u32;
    let max_color_attachments = get_usize(gl, glow::MAX_COLOR_ATTACHMENTS).unwrap_or(8) as u8;

    let mut limits = Limits {
        max_image_1d_size: max_texture_size,
        max_image_2d_size: max_texture_size,
        max_image_3d_size: max_texture_size,
        max_image_cube_size: max_texture_size,
        max_image_array_layers: get_usize(gl, glow::MAX_ARRAY_TEXTURE_LAYERS).unwrap_or(1) as u16,
        max_texel_elements: get_usize(gl, glow::MAX_TEXTURE_BUFFER_SIZE).unwrap_or(0),
        max_viewports: 1,
        optimal_buffer_copy_offset_alignment: 1,
        optimal_buffer_copy_pitch_alignment: 1,
        min_texel_buffer_offset_alignment: 1,   // TODO
        min_uniform_buffer_offset_alignment: 1, // TODO
        min_storage_buffer_offset_alignment: 1, // TODO
        framebuffer_color_samples_count: max_color_attachments,
        ..Limits::default()
    };

    if info.is_supported(&[Core(4, 0), Ext("GL_ARB_tessellation_shader")]) {
        limits.max_patch_size = get_usize(gl, glow::MAX_PATCH_VERTICES).unwrap_or(0) as _;
    }
    if info.is_supported(&[Core(4, 1)]) {
        // TODO: extension
        limits.max_viewports = get_usize(gl, glow::MAX_VIEWPORTS).unwrap_or(0);
    }

    if false
        && info.is_supported(&[
            //TODO: enable when compute is implemented
            Core(4, 3),
            Ext("GL_ARB_compute_shader"),
        ])
    {
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

    let mut features = Features::empty();
    let mut legacy = LegacyFeatures::empty();

    if info.is_supported(&[
        Core(4, 6),
        Ext("GL_ARB_texture_filter_anisotropic"),
        Ext("GL_EXT_texture_filter_anisotropic"),
    ]) {
        features |= Features::SAMPLER_ANISOTROPY;
    }
    if info.is_supported(&[Core(4, 2)]) {
        legacy |= LegacyFeatures::EXPLICIT_LAYOUTS_IN_SHADER;
    }
    if info.is_supported(&[Core(3, 3), Es(3, 0), Ext("GL_ARB_instanced_arrays")]) {
        features |= Features::INSTANCE_RATE;
    }
    if info.is_supported(&[Core(3, 3)]) {
        // TODO: extension
        features |= Features::SAMPLER_MIP_LOD_BIAS;
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
    if info.is_supported(&[Core(4, 2)]) {
        // TODO: extension
        legacy |= LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE;
    }
    if info.is_supported(&[
        Core(3, 2),
        Es(3, 2),
        Ext("GL_ARB_draw_elements_base_vertex"),
    ]) {
        legacy |= LegacyFeatures::VERTEX_BASE;
    }
    if info.is_supported(&[Core(3, 2), Ext("GL_ARB_framebuffer_sRGB")]) {
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
    if info.is_supported(&[Core(3, 3)]) {
        // TODO: extension
        legacy |= LegacyFeatures::SAMPLER_BORDER_COLOR;
    }
    if info.is_supported(&[Core(3, 3), Es(3, 0)]) {
        legacy |= LegacyFeatures::INSTANCED_ATTRIBUTE_BINDING;
    }

    let emulate_map = true; //info.version.is_embedded;

    let private = PrivateCaps {
        vertex_array: info.is_supported(&[Core(3, 0), Es(3, 0), Ext("GL_ARB_vertex_array_object")]),
        // TODO && gl.GenVertexArrays.is_loaded(),
        framebuffer: info.is_supported(&[Core(3, 0), Es(2, 0), Ext("GL_ARB_framebuffer_object")]),
        // TODO && gl.GenFramebuffers.is_loaded(),
        framebuffer_texture: info.is_supported(&[Core(3, 0)]), //TODO: double check
        buffer_role_change: true || !info.version.is_embedded, // TODO
        image_storage: info.is_supported(&[Core(4, 2), Ext("GL_ARB_texture_storage")]),
        buffer_storage: info.is_supported(&[Core(4, 4), Ext("GL_ARB_buffer_storage")]),
        clear_buffer: info.is_supported(&[Core(3, 0), Es(3, 0)]),
        program_interface: info.is_supported(&[Core(4, 3), Ext("GL_ARB_program_interface_query")]),
        frag_data_location: !info.version.is_embedded,
        sync: false && info.is_supported(&[Core(3, 2), Es(3, 0), Ext("GL_ARB_sync")]), // TODO
        map: !info.version.is_embedded, //TODO: OES extension
        sampler_anisotropy_ext: !info
            .is_supported(&[Core(4, 6), Ext("GL_ARB_texture_filter_anisotropic")])
            && info.is_supported(&[Ext("GL_EXT_texture_filter_anisotropic")]),
        emulate_map, // TODO
        depth_range_f64_precision: !info.version.is_embedded, // TODO
        draw_buffers: !info.version.is_embedded, // TODO
    };

    (info, features, legacy, limits, private)
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn test_version_parse() {
        assert_eq!(Version::parse("1".to_string()), Err("1".to_string()));
        assert_eq!(Version::parse("1.".to_string()), Err("1.".to_string()));
        assert_eq!(Version::parse("1 h3l1o. W0rld".to_string()), Err("1 h3l1o. W0rld".to_string()));
        assert_eq!(Version::parse("1. h3l1o. W0rld".to_string()), Err("1. h3l1o. W0rld".to_string()));
        assert_eq!(Version::parse("1.2.3".to_string()), Ok(Version::new(1, 2, Some(3), "".to_string())));
        assert_eq!(Version::parse("1.2".to_string()), Ok(Version::new(1, 2, None, "".to_string())));
        assert_eq!(
            Version::parse("1.2 h3l1o. W0rld".to_string()),
            Ok(Version::new(1, 2, None, "h3l1o. W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2.h3l1o. W0rld".to_string()),
            Ok(Version::new(1, 2, None, "W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2. h3l1o. W0rld".to_string()),
            Ok(Version::new(1, 2, None, "h3l1o. W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2.3.h3l1o. W0rld".to_string()),
            Ok(Version::new(1, 2, Some(3), "W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("1.2.3 h3l1o. W0rld".to_string()),
            Ok(Version::new(1, 2, Some(3), "h3l1o. W0rld".to_string()))
        );
        assert_eq!(
            Version::parse("OpenGL ES 3.1".to_string()),
            Ok(Version::new_embedded(3, 1, "".to_string()))
        );
        assert_eq!(
            Version::parse("OpenGL ES 2.0 Google Nexus".to_string()),
            Ok(Version::new_embedded(2, 0, "Google Nexus".to_string()))
        );
        assert_eq!(
            Version::parse("GLSL ES 1.1".to_string()),
            Ok(Version::new_embedded(1, 1, "".to_string()))
        );
    }
}
