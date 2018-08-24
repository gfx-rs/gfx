use std::collections::HashSet;
use std::{ffi, fmt, mem, str};
use gl;
use hal::{Features, Limits};

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

fn get_usize(gl: &gl::Gl, name: gl::types::GLenum) -> usize {
    let mut value = 0 as gl::types::GLint;
    unsafe { gl.GetIntegerv(name, &mut value) };
    value as usize
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
        /// Support explicit layouts in shader.
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
            let num_exts = get_usize(gl, gl::NUM_EXTENSIONS) as gl::types::GLuint;
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
        max_texture_size: get_usize(gl, gl::MAX_TEXTURE_SIZE),
        max_texel_elements: get_usize(gl, gl::MAX_TEXTURE_BUFFER_SIZE),
        max_viewports: 1,
        min_buffer_copy_offset_alignment: 1,
        min_buffer_copy_pitch_alignment: 1,
        min_texel_buffer_offset_alignment: 1, // TODO
        min_uniform_buffer_offset_alignment: 1, // TODO
        min_storage_buffer_offset_alignment: 1, // TODO
        .. Limits::default()
    };

    if info.is_supported(&[
        Core(4,0),
        Ext("GL_ARB_tessellation_shader"),
    ]) {
        limits.max_patch_size = get_usize(gl, gl::MAX_PATCH_VERTICES) as _;
    }
    if info.is_supported(&[Core(4,1)]) { // TODO: extension
        limits.max_viewports = get_usize(gl, gl::MAX_VIEWPORTS);
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
            unsafe {
                gl.GetIntegeri_v(gl::MAX_COMPUTE_WORK_GROUP_COUNT, i as _, &mut values[0]);
                gl.GetIntegeri_v(gl::MAX_COMPUTE_WORK_GROUP_SIZE, i as _, &mut values[1]);
            }
            *count = values[0] as _;
            *size = values[1] as _;
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
