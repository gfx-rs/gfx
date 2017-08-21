// Copyright 2017 The Gfx-rs Developers.
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

//! Shader handling.

use std::error::Error;
use std::fmt;

/// Shader pipeline stage
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Pixel,
    Compute,
}

bitflags!(
    /// Combination of different shader pipeline stages.
    pub flags StageFlags: u16 {
        const STAGE_VERTEX   = 0x1,
        const STAGE_HULL     = 0x2,
        const STAGE_DOMAIN   = 0x4,
        const STAGE_GEOMETRY = 0x8,
        const STAGE_PIXEL    = 0x10,
        const STAGE_COMPUTE  = 0x20,
        const STAGE_GRAPHICS = STAGE_VERTEX.bits | STAGE_HULL.bits |
            STAGE_DOMAIN.bits | STAGE_GEOMETRY.bits | STAGE_PIXEL.bits,
        const STAGE_ALL      = STAGE_GRAPHICS.bits | STAGE_COMPUTE.bits,
    }
);

/// An error type for creating shaders.
#[derive(Clone, PartialEq, Debug)]
pub enum CreateShaderError {
    /// The device does not support the requested shader model.
    ModelNotSupported,
    /// The device does not support the shader stage.
    StageNotSupported(Stage),
    /// The shader failed to compile.
    CompilationFailed(String),
    /// Library source type is not supported.
    LibrarySourceNotSupported,
}

impl fmt::Display for CreateShaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreateShaderError::ModelNotSupported |
            CreateShaderError::LibrarySourceNotSupported |
            CreateShaderError::CompilationFailed(_) => f.pad(self.description()),
            CreateShaderError::StageNotSupported(ref stage) => {
                write!(f, "the device does not support the {:?} stage", stage)
            }
        }
    }
}

impl Error for CreateShaderError {
    fn description(&self) -> &str {
        match *self {
            CreateShaderError::ModelNotSupported => "the device does not support the requested shader model",
            CreateShaderError::LibrarySourceNotSupported => "the library source type is not supported",
            CreateShaderError::CompilationFailed(ref err_msg) => err_msg,
            CreateShaderError::StageNotSupported(_) => "the device does not support the specified stage",
        }
    }
}
