// Copyright 2016 The Gfx-rs Developers.
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

use gfx_device_gl::info::Version as GlslVersion;
use gfx_device_dx11::ShaderModel as DxShaderModel;

/// Shader backend with version numbers.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Backend {
    Glsl(GlslVersion),
    Hlsl(DxShaderModel),
}

/// A type storing shader source for different graphics APIs and versions.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Source<'a> {
    pub glsl_120: Option<&'a [u8]>,
    pub glsl_130: Option<&'a [u8]>,
    pub glsl_140: Option<&'a [u8]>,
    pub glsl_150: Option<&'a [u8]>,
    pub glsl_430: Option<&'a [u8]>,
    pub hlsl_30 : Option<&'a [u8]>,
    pub hlsl_40 : Option<&'a [u8]>,
    pub hlsl_41 : Option<&'a [u8]>,
    pub hlsl_50 : Option<&'a [u8]>,
}

impl<'a> Source<'a> {
    /// Create an empty shader source. Useful for specifying the remaining
    /// structure members upon construction.
    pub fn empty() -> Source<'a> {
        ShaderSource {
            glsl_120: None,
            glsl_130: None,
            glsl_140: None,
            glsl_150: None,
            glsl_430: None,
            hlsl_30: None,
            hlsl_40: None,
            hlsl_41: None,
            hlsl_50: None,
        }
    }

    /// Pick one of the stored versions that is the highest supported by the backend.
    pub fn select(&self, backend: Bakend) -> Result<&'a [u8], ()> {
        Ok(match backend {
            Backend::Glsl(version) => {
                let v = version.major * 100 + version.minor * 10;
                match *self {
                    Source { glsl_430: Some(s), .. } if v >= 430 => s,
                    Source { glsl_150: Some(s), .. } if v >= 150 => s,
                    Source { glsl_140: Some(s), .. } if v >= 140 => s,
                    Source { glsl_130: Some(s), .. } if v >= 130 => s,
                    Source { glsl_120: Some(s), .. } if v >= 120 => s,
                    _ => return Err(())
                },
            },
            Backend::Hlsl(model) = match *self {
                Source { hlsl_50: Some(s), .. } if model >= 50 => s,
                Source { hlsl_41: Some(s), .. } if model >= 41 => s,
                Source { hlsl_40: Some(s), .. } if model >= 40 => s,
                Source { hlsl_30: Some(s), .. } if model >= 30 => s,
                _ => return Err(())
            },
        })
    }
}
