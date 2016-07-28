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


pub use gfx_device_gl::Version as GlslVersion;
#[cfg(target_os = "windows")]
pub use gfx_device_dx11::ShaderModel as DxShaderModel;
#[cfg(target_os = "macos")]
pub use gfx_device_metal::ShaderModel as MetalShaderModel;

/// Shader backend with version numbers.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Backend {
    Glsl(GlslVersion),
    GlslEs(GlslVersion),
    #[cfg(target_os = "windows")]
    Hlsl(DxShaderModel),
    #[cfg(target_os = "macos")]
    Msl(MetalShaderModel)
}

pub const EMPTY: &'static [u8] = &[];

/// A type storing shader source for different graphics APIs and versions.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Source<'a> {
    pub glsl_120: &'a [u8],
    pub glsl_130: &'a [u8],
    pub glsl_140: &'a [u8],
    pub glsl_150: &'a [u8],
    pub glsl_430: &'a [u8],
    pub glsl_es_100: &'a [u8],
    pub glsl_es_200: &'a [u8],
    pub glsl_es_300: &'a [u8],
    pub hlsl_30 : &'a [u8],
    pub hlsl_40 : &'a [u8],
    pub hlsl_41 : &'a [u8],
    pub hlsl_50 : &'a [u8],
    pub msl_10  : &'a [u8],
    pub msl_11  : &'a [u8]
}

impl<'a> Source<'a> {
    /// Create an empty shader source. Useful for specifying the remaining
    /// structure members upon construction.
    pub fn empty() -> Source<'a> {
        Source {
            glsl_120: EMPTY,
            glsl_130: EMPTY,
            glsl_140: EMPTY,
            glsl_150: EMPTY,
            glsl_430: EMPTY,
            glsl_es_100: EMPTY,
            glsl_es_200: EMPTY,
            glsl_es_300: EMPTY,
            hlsl_30:  EMPTY,
            hlsl_40:  EMPTY,
            hlsl_41:  EMPTY,
            hlsl_50:  EMPTY,
            msl_10:   EMPTY,
            msl_11:   EMPTY
        }
    }

    /// Pick one of the stored versions that is the highest supported by the backend.
    pub fn select(&self, backend: Backend) -> Result<&'a [u8], ()> {
        Ok(match backend {
            Backend::Glsl(version) => {
                let v = version.major * 100 + version.minor;
                match *self {
                    Source { glsl_430: s, .. } if s != EMPTY && v >= 430 => s,
                    Source { glsl_150: s, .. } if s != EMPTY && v >= 150 => s,
                    Source { glsl_140: s, .. } if s != EMPTY && v >= 140 => s,
                    Source { glsl_130: s, .. } if s != EMPTY && v >= 130 => s,
                    Source { glsl_120: s, .. } if s != EMPTY && v >= 120 => s,
                    _ => return Err(())
                }
            },
            Backend::GlslEs(version) => {
                let v = version.major * 100 + version.minor;
                match *self {
                    Source { glsl_es_100: s, .. } if s != EMPTY && v >= 100 => s,
                    Source { glsl_es_200: s, .. } if s != EMPTY && v >= 200 => s,
                    Source { glsl_es_300: s, .. } if s != EMPTY && v >= 300 => s,
                    _ => return Err(())
                }
            },
            #[cfg(target_os = "windows")]
            Backend::Hlsl(model) => match *self {
                Source { hlsl_50: s, .. } if s != EMPTY && model >= 50 => s,
                Source { hlsl_41: s, .. } if s != EMPTY && model >= 41 => s,
                Source { hlsl_40: s, .. } if s != EMPTY && model >= 40 => s,
                Source { hlsl_30: s, .. } if s != EMPTY && model >= 30 => s,
                _ => return Err(())
            },
            #[cfg(target_os = "macos")]
            Backend::Msl(revision) => match *self {
                Source { msl_11: s, .. } if s != EMPTY && revision >= 11 => s,
                Source { msl_10: s, .. } if s != EMPTY && revision >= 10 => s,
                _ => return Err(())
            }
        })
    }
}
