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
use std::ffi;
use std::fmt;
use std::mem;
use std::str;
use super::gl;

use gfx::device::Capabilities;
use gfx::device::shade;

/// A version number for a specific component of an OpenGL implementation
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub revision: Option<u32>,
    pub vendor_info: &'static str,
}

// FIXME https://github.com/rust-lang/rust/issues/18738
// derive

#[automatically_derived]
impl ::std::cmp::Ord for Version {
    #[inline]
    fn cmp(&self, other: &Version) -> ::std::cmp::Ordering {
        (&self.major, &self.minor, &self.revision, self.vendor_info)
            .cmp(&(&other.major, &other.minor, &other.revision, other.vendor_info))
    }
}
#[automatically_derived]
impl ::std::cmp::PartialOrd for Version {
    #[inline]
    fn partial_cmp(&self, other: &Version) -> ::std::option::Option<::std::cmp::Ordering> {
        (&self.major, &self.minor, &self.revision, self.vendor_info)
            .partial_cmp(&(&other.major, &other.minor, &other.revision, other.vendor_info))
    }
}

impl Version {
    /// Create a new OpenGL version number
    pub fn new(major: u32, minor: u32, revision: Option<u32>,
               vendor_info: &'static str) -> Version {
        Version {
            major: major,
            minor: minor,
            revision: revision,
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
    pub fn parse(src: &'static str) -> Result<Version, &'static str> {
        let (version, vendor_info) = match src.find(' ') {
            Some(i) => (&src[..i], &src[(i + 1)..]),
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

    /// Returns `true` if the implementation supports the extension
    pub fn is_extension_supported(&self, s: &'static str) -> bool {
        self.extensions.contains(&s)
    }

    pub fn is_version_or_extension_supported(&self, major: u32, minor: u32, ext: &'static str) -> bool {
        self.version >= Version::new(major, minor, None, "") || self.is_extension_supported(ext)
    }
}

fn to_shader_model(v: &Version) -> shade::ShaderModel {
    use gfx::device::shade::ShaderModel;
    match v {
        v if *v < Version::new(1, 20, None, "") => ShaderModel::Unsupported,
        v if *v < Version::new(1, 50, None, "") => ShaderModel::Version30,
        v if *v < Version::new(3,  0, None, "") => ShaderModel::Version40,
        v if *v < Version::new(4, 30, None, "") => ShaderModel::Version41,
        _                                       => ShaderModel::Version50,
    }
}

/// Load the information pertaining to the driver and the corresponding device
/// capabilities.
pub fn get(gl: &gl::Gl) -> (Info, Capabilities) {
    let info = Info::get(gl);
    let caps = Capabilities {
        shader_model:                   to_shader_model(&info.shading_language),

        max_vertex_count:               get_usize(gl, gl::MAX_ELEMENTS_VERTICES),
        max_index_count:                get_usize(gl, gl::MAX_ELEMENTS_INDICES),
        max_draw_buffers:               get_usize(gl, gl::MAX_DRAW_BUFFERS),
        max_texture_size:               get_usize(gl, gl::MAX_TEXTURE_SIZE),
        max_vertex_attributes:          get_usize(gl, gl::MAX_VERTEX_ATTRIBS),

        buffer_role_change_allowed:     true, //TODO

        array_buffer_supported:         info.is_version_or_extension_supported(3, 0, "GL_ARB_vertex_array_object"),
        fragment_output_supported:      info.is_version_or_extension_supported(3, 0, "GL_EXT_gpu_shader4"),
        immutable_storage_supported:    info.is_version_or_extension_supported(4, 2, "GL_ARB_texture_storage"),
        instance_base_supported:        info.is_version_or_extension_supported(4, 2, "GL_ARB_base_instance"),
        instance_call_supported:        info.is_version_or_extension_supported(3, 1, "GL_ARB_draw_instanced"),
        instance_rate_supported:        info.is_version_or_extension_supported(3, 3, "GL_ARB_instanced_arrays"),
        render_targets_supported:       info.is_version_or_extension_supported(3, 0, "GL_ARB_framebuffer_object"),
        srgb_color_supported:           info.is_version_or_extension_supported(3, 2, "GL_ARB_framebuffer_sRGB"),
        sampler_objects_supported:      info.is_version_or_extension_supported(3, 3, "GL_ARB_sampler_objects"),
        uniform_block_supported:        info.is_version_or_extension_supported(3, 0, "GL_ARB_uniform_buffer_object"),
        vertex_base_supported:          info.is_version_or_extension_supported(3, 2, "GL_ARB_draw_elements_base_vertex"),
    };
    (info, caps)
}

#[cfg(test)]
mod tests {
    use super::Version;
    use super::to_shader_model;

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
    }

    #[test]
    fn test_shader_model() {
        use gfx::device::shade::ShaderModel;
        assert_eq!(to_shader_model(&Version::parse("1.10").unwrap()), ShaderModel::Unsupported);
        assert_eq!(to_shader_model(&Version::parse("1.20").unwrap()), ShaderModel::Version30);
        assert_eq!(to_shader_model(&Version::parse("1.50").unwrap()), ShaderModel::Version40);
        assert_eq!(to_shader_model(&Version::parse("3.00").unwrap()), ShaderModel::Version41);
        assert_eq!(to_shader_model(&Version::parse("4.30").unwrap()), ShaderModel::Version50);
    }
}
