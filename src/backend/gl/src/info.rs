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
    pub fn new(major: u32, minor: u32, revision: Option<u32>,
               vendor_info: &'static str) -> Version {
        Version {
            is_embedded: false,
            major: major,
            minor: minor,
            revision: revision,
            vendor_info: vendor_info,
        }
    }
    /// Create a new OpenGL ES version number
    pub fn new_embedded(major: u32, minor: u32, vendor_info: &'static str) -> Version {
        Version {
            is_embedded: true,
            major: major,
            minor: minor,
            revision: None,
            vendor_info: vendor_info,
        }
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
    fn get(gl: &gl::Gl) -> PlatformName {
        PlatformName {
            vendor: get_string(gl, gl::VENDOR),
            renderer: get_string(gl, gl::RENDERER),
        }
    }
}

/// Private capabilities that don't need to be exposed.
#[derive(Debug)]
pub struct PrivateCaps {
    /// VAO support
    pub vertex_array: bool,
    /// FBO support
    pub framebuffer: bool,
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
pub fn query_all(gl: &gl::Gl) -> (Info, Features, Limits, PrivateCaps) {
    use self::Requirement::*;
    let info = Info::get(gl);
    let tessellation_supported =           info.is_supported(&[Core(4,0),
                                                               Ext("GL_ARB_tessellation_shader")]);
    let multi_viewports_supported =        info.is_supported(&[Core(4,1)]); // TODO: extension
    let compute_supported =                info.is_supported(&[Core(4,3),
                                                               Ext("GL_ARB_compute_shader")]);
    let mut max_compute_group_count = [0usize; 3];
    let mut max_compute_group_size = [0usize; 3];
    if compute_supported {
        let mut values = [0 as gl::types::GLint; 2];
        for (i, (count, size)) in max_compute_group_count
            .iter_mut()
            .zip(max_compute_group_size.iter_mut())
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

    let limits = Limits {
        max_texture_size: get_usize(gl, gl::MAX_TEXTURE_SIZE),
        max_patch_size: if tessellation_supported { get_usize(gl, gl::MAX_PATCH_VERTICES) as u8 } else {0},
        max_viewports: if multi_viewports_supported { get_usize(gl, gl::MAX_VIEWPORTS) } else {1},
        max_compute_group_count,
        max_compute_group_size,

        min_buffer_copy_offset_alignment: 1,
        min_buffer_copy_pitch_alignment: 1,
        min_uniform_buffer_offset_alignment: 1, // TODO

    };
    let features = Features {
        indirect_execution:                 info.is_supported(&[Core(4,3),
                                                                Es  (3,1)]), // TODO: extension
        draw_instanced:                     info.is_supported(&[Core(3,1),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_draw_instanced")]),
        draw_instanced_base:                info.is_supported(&[Core(4,2),
                                                                Ext ("GL_ARB_base_instance")]),
        draw_indexed_base:                  info.is_supported(&[Core(3,2)]), // TODO: extension
        draw_indexed_instanced:             info.is_supported(&[Core(3,1),
                                                                Es  (3,0)]), // TODO: extension
        draw_indexed_instanced_base_vertex: info.is_supported(&[Core(3,2)]), // TODO: extension
        draw_indexed_instanced_base:        info.is_supported(&[Core(4,2)]), // TODO: extension
        instance_rate:                      info.is_supported(&[Core(3,3),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_instanced_arrays")]),
        vertex_base:                        info.is_supported(&[Core(3,2),
                                                                Es  (3,2),
                                                                Ext ("GL_ARB_draw_elements_base_vertex")]),
        srgb_color:                         info.is_supported(&[Core(3,2),
                                                                Ext ("GL_ARB_framebuffer_sRGB")]),
        constant_buffer:                    info.is_supported(&[Core(3,1),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_uniform_buffer_object")]),
        unordered_access_view:              info.is_supported(&[Core(4,0)]), // TODO: extension
        separate_blending_slots:            info.is_supported(&[Core(4,0),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_draw_buffers_blend")]),
        copy_buffer:                        info.is_supported(&[Core(3,1),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_copy_buffer"),
                                                                Ext ("GL_NV_copy_buffer")]),
        sampler_objects:                    info.is_supported(&[Core(3,3),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_sampler_objects")]),
        sampler_lod_bias:                   info.is_supported(&[Core(3,3)]), // TODO: extension
        sampler_anisotropy:                 info.is_supported(&[Core(4,6),
                                                                Ext ("GL_ARB_texture_filter_anisotropic"),
                                                                Ext ("GL_EXT_texture_filter_anisotropic")]),
        sampler_border_color:               info.is_supported(&[Core(3,3)]), // TODO: extensions
    };
    let private = PrivateCaps {
        vertex_array:                       info.is_supported(&[Core(3,0),
                                                                Es  (3,0),
                                                                Ext ("GL_ARB_vertex_array_object")]),
        framebuffer:                        info.is_supported(&[Core(3,0),
                                                                Es  (2,0),
                                                                Ext ("GL_ARB_framebuffer_object")]),
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

    (info, features, limits, private)
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
