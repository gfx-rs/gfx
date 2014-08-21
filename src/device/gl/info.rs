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
use std::fmt;
use std::str;
use super::gl;

use Capabilities;
use shade;

/// A version number for a specific component of an OpenGL implementation
#[deriving(Eq, PartialEq, Ord, PartialOrd)]
pub struct Version {
    pub major: uint,
    pub minor: uint,
    pub revision: Option<uint>,
    pub vendor_info: &'static str,
}

impl Version {
    /// Create a new OpenGL version number
    pub fn new(major: uint, minor: uint, revision: Option<uint>,
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
            Some(i) => (src.slice_to(i), src.slice_from(i + 1)),
            None => (src, ""),
        };

        // TODO: make this even more lenient so that we can also accept
        // `<major> "." <minor> [<???>]`
        let mut it = version.split('.');
        let major = it.next().and_then(from_str);
        let minor = it.next().and_then(from_str);
        let revision = it.next().and_then(from_str);

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

impl fmt::Show for Version {
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

/// Get a statically allocated string from the implementation using
/// `glGetString`. Fails if it `GLenum` cannot be handled by the
/// implementation's `gl.GetString` function.
fn get_string(gl: &gl::Gl, name: gl::types::GLenum) -> &'static str {
    let ptr = gl.GetString(name) as *const i8;
    if !ptr.is_null() {
        // This should be safe to mark as statically allocated because
        // GlGetString only returns static strings.
        unsafe { str::raw::c_str_to_static_slice(ptr) }
    } else {
        fail!("Invalid GLenum passed to `get_string`: {:x}", name)
    }
}

fn get_uint(gl: &gl::Gl, name: gl::types::GLenum) -> uint {
    let mut value = 0 as gl::types::GLint;
    unsafe { gl.GetIntegerv(name, &mut value) };
    value as uint
}

/// A unique platform identifier that does not change between releases
#[deriving(Eq, PartialEq, Show)]
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
#[deriving(Show)]
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
        let extensions = if version >= Version::new(3, 2, None, "") {
            let num_exts = get_uint(gl, gl::NUM_EXTENSIONS) as gl::types::GLuint;
            range(0, num_exts).map(|i| {
                unsafe {
                    str::raw::c_str_to_static_slice(
                        gl.GetStringi(gl::EXTENSIONS, i) as *const i8,
                    )
                }
            }).collect()
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
    pub fn is_extension_supported(&self, s: &str) -> bool {
        self.extensions.contains_equiv(&s)
    }
}

fn to_shader_model(v: &Version) -> shade::ShaderModel {
    match *v {
        v if v < Version::new(1, 20, None, "") => shade::ModelUnsupported,
        v if v < Version::new(1, 50, None, "") => shade::Model30,
        v if v < Version::new(3,  0, None, "") => shade::Model40,
        v if v < Version::new(4, 30, None, "") => shade::Model41,
        _                                      => shade::Model50,
    }
}

/// Load the information pertaining to the driver and the corresponding device
/// capabilities.
pub fn get(gl: &gl::Gl) -> (Info, Capabilities) {
    let info = Info::get(gl);
    let caps = Capabilities {
        shader_model: to_shader_model(&info.shading_language),
        max_draw_buffers: get_uint(gl, gl::MAX_DRAW_BUFFERS),
        max_texture_size: get_uint(gl, gl::MAX_TEXTURE_SIZE),
        max_vertex_attributes: get_uint(gl, gl::MAX_VERTEX_ATTRIBS),
        uniform_block_supported: info.version >= Version::new(3, 1, None, "")
            || info.is_extension_supported("GL_ARB_uniform_buffer_object"),
        array_buffer_supported: info.version >= Version::new(3, 0, None, "")
            || info.is_extension_supported("GL_ARB_vertex_array_object"),
        immutable_storage_supported: info.version >= Version::new(4, 2, None, "")
            || info.is_extension_supported("GL_ARB_texture_storage"),
        sampler_objects_supported: info.version >= Version::new(3, 3, None, "")
            || info.is_extension_supported("GL_ARB_sampler_objects"),
    };
    (info, caps)
}

#[cfg(test)]
mod tests {
    use super::Version;
    use super::to_shader_model;
    use shade;

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
        assert_eq!(to_shader_model(&Version::parse("1.10").unwrap()), shade::ModelUnsupported);
        assert_eq!(to_shader_model(&Version::parse("1.20").unwrap()), shade::Model30);
        assert_eq!(to_shader_model(&Version::parse("1.50").unwrap()), shade::Model40);
        assert_eq!(to_shader_model(&Version::parse("3.00").unwrap()), shade::Model41);
        assert_eq!(to_shader_model(&Version::parse("4.30").unwrap()), shade::Model50);
    }
}
