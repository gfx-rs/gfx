// Copyright 2014 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashSet;
use std::{ffi, fmt, mem, str};
use gl;
use core::Capabilities;


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
    pub array_buffer_supported: bool,
    pub frame_buffer_supported: bool,
    pub immutable_storage_supported: bool,
    pub sampler_objects_supported: bool,
    pub program_interface_supported: bool,
    pub buffer_storage_supported: bool,
    pub clear_buffer_supported: bool,
    pub frag_data_location_supported: bool,
}

/// OpenGL implementation information
#[derive(Debug)]
pub struct Info {
    /// The platform identifier
    pub platform_name: PlatformName,
    /// The OpenGL API vesion number
    pub version: Version,
    /// The GLSL vesion number
    pub shading_language: Version,
    /// The extensions supported by the implementation
    pub extensions: HashSet<&'static str>,
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
}

/// Load the information pertaining to the driver and the corresponding device
/// capabilities.
pub fn get(gl: &gl::Gl) -> (Info, Capabilities, PrivateCaps) {
    let info = Info::get(gl);
    let tessellation_supported =           info.is_version_or_extension_supported(4, 0, "GL_ARB_tessellation_shader");
    let caps = Capabilities {
        max_vertex_count: get_usize(gl, gl::MAX_ELEMENTS_VERTICES),
        max_index_count:  get_usize(gl, gl::MAX_ELEMENTS_INDICES),
        max_texture_size: get_usize(gl, gl::MAX_TEXTURE_SIZE),
        max_patch_size: if tessellation_supported { get_usize(gl, gl::MAX_PATCH_VERTICES) } else {0},

        instance_base_supported:           info.is_version_or_extension_supported(4, 2, "GL_ARB_base_instance"),
        instance_call_supported:           info.is_version_or_extension_supported(3, 1, "GL_ARB_draw_instanced"),
        instance_rate_supported:           info.is_version_or_extension_supported(3, 3, "GL_ARB_instanced_arrays"),
        vertex_base_supported:             info.is_version_or_extension_supported(3, 2, "GL_ARB_draw_elements_base_vertex"),
        srgb_color_supported:              info.is_version_or_extension_supported(3, 2, "GL_ARB_framebuffer_sRGB"),
        constant_buffer_supported:         info.is_version_or_extension_supported(3, 1, "GL_ARB_uniform_buffer_object"),
        unordered_access_view_supported:   info.is_version_supported(4, 0), //TODO: extension
        separate_blending_slots_supported: info.is_version_or_extension_supported(4, 0, "GL_ARB_draw_buffers_blend"),
        copy_buffer_supported:             info.is_version_or_extension_supported(3, 1, "GL_ARB_copy_buffer") |
                                           info.is_embedded_version_supported(3, 0) |
                                          (info.is_embedded_version_supported(2, 0) & info.is_extension_supported("GL_NV_copy_buffer")),
    };
    let private = PrivateCaps {
        array_buffer_supported:            info.is_version_or_extension_supported(3, 0, "GL_ARB_vertex_array_object"),
        frame_buffer_supported:            info.is_version_or_extension_supported(3, 0, "GL_ARB_framebuffer_object") |
                                           info.is_embedded_version_supported(2, 0),
        immutable_storage_supported:       info.is_version_or_extension_supported(4, 2, "GL_ARB_texture_storage"),
        sampler_objects_supported:         info.is_version_or_extension_supported(3, 3, "GL_ARB_sampler_objects"),
        program_interface_supported:       info.is_version_or_extension_supported(4, 3, "GL_ARB_program_interface_query"),
        buffer_storage_supported:          info.is_version_or_extension_supported(4, 4, "GL_ARB_buffer_storage"),
        clear_buffer_supported:            info.is_version_supported(3, 0) | info.is_embedded_version_supported(3, 0),
        frag_data_location_supported:      !info.version.is_embedded,
    };
    (info, caps, private)
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
