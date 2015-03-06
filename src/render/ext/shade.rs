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

//! Shader source extension

use device::shade::{CreateShaderError, ShaderModel};

/// Program linking error
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ProgramError {
    /// Unable to compile the vertex shader
    Vertex(CreateShaderError),
    /// Unable to compile the fragment shader
    Fragment(CreateShaderError),
    /// Unable to link
    Link(()),
}

/// A type storing shader source for different graphics APIs and versions.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ShaderSource<'a> {
    pub glsl_120: Option<&'a [u8]>,
    pub glsl_130: Option<&'a [u8]>,
    pub glsl_140: Option<&'a [u8]>,
    pub glsl_150: Option<&'a [u8]>,
    pub glsl_430: Option<&'a [u8]>,
    // TODO: hlsl_sm_N...
    pub targets: &'a [&'a str],
}

impl<'a> ShaderSource<'a> {
    /// Create an empty shader source. Useful for specifying the remaining
    /// structure members upon construction.
    pub fn empty() -> ShaderSource<'a> {
        ShaderSource {
            glsl_120: None,
            glsl_130: None,
            glsl_140: None,
            glsl_150: None,
            glsl_430: None,
            targets: &[],
        }
    }

    /// Pick one of the stored versions that is the highest supported by the device.
    pub fn choose(&self, model: ShaderModel) -> Result<&'a [u8], ()> {
        // following https://www.opengl.org/wiki/Detecting_the_Shader_Model
        let version = model.to_number();
        Ok(match *self {
            ShaderSource { glsl_430: Some(s), .. } if version >= 50 => s,
            ShaderSource { glsl_150: Some(s), .. } if version >= 40 => s,
            ShaderSource { glsl_140: Some(s), .. } if version >= 40 => s,
            ShaderSource { glsl_130: Some(s), .. } if version >= 30 => s,
            ShaderSource { glsl_120: Some(s), .. } if version >= 20 => s,
            _ => return Err(()),
        })
    }
}