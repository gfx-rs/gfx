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

//! Raw Pipeline State Objects
//!
//! This module contains items used to create and manage a raw pipeline state object. Most users
//! will want to use the typed and safe `PipelineState`. See the `pso` module inside the `gfx`
//! crate.

use std::error::Error;
use std::fmt;

mod descriptor;
mod graphics;
mod input_assembler;
mod output_merger;

pub use self::descriptor::*;
pub use self::graphics::*;
pub use self::input_assembler::*;
pub use self::output_merger::*;

/// Error types happening upon PSO creation on the device side.
#[derive(Clone, Debug, PartialEq)]
pub struct CreationError;

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        "Could not create PSO on device."
    }
}

/// Shader entry point.
pub type EntryPoint = &'static str;

bitflags!(
    /// Stages of the logical pipeline.
    ///
    /// The pipeline is structured as given the by the ordering of the flags.
    /// Some stages are queue type dependent.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub flags PipelineStage: u32 {
        /// Beginning of the command queue.
        const TOP_OF_PIPE = 0x1,
        /// Indirect data consumption.
        const DRAW_INDIRECT = 0x2,
        /// Vertex data consumption.
        const VERTEX_INPUT = 0x4,
        /// Vertex shader execution.
        const VERTEX_SHADER = 0x8,
        /// Hull shader execution.
        const HULL_SHADER = 0x10,
        /// Domain shader execution.
        const DOMAIN_SHADER = 0x20,
        /// Geometry shader execution.
        const GEOMETRY_SHADER = 0x40,
        /// Pixel shader execution.
        const PIXEL_SHADER = 0x80,
        /// Stage of early depth and stencil test.
        const EARLY_FRAGMENT_TESTS = 0x100,
        /// Stage of late depth and stencil test.
        const LATE_FRAGMENT_TESTS = 0x200,
        /// Stage of final color value calculation.
        const COLOR_ATTACHMENT_OUTPUT = 0x400,
        /// Compute shader execution,
        const COMPUTE_SHADER = 0x800,
        /// Copy/Transfer command execution.
        const TRANSFER = 0x1000,
        /// End of the command queue.
        const BOTTOM_OF_PIPE = 0x2000,
        /// Read/Write access from host.
        /// (Not a real pipeline stage)
        const HOST = 0x4000,
    }
);

bitflags!(
    /// Combination of different shader pipeline stages.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub flags ShaderStageFlags: u16 {
        /// Vertex shader stage.
        const STAGE_VERTEX   = 0x1,
        /// Hull (tessellation) shader stage.
        const STAGE_HULL     = 0x2,
        /// Domain (tessellation) shader stage.
        const STAGE_DOMAIN   = 0x4,
        /// Geometry shader stage.
        const STAGE_GEOMETRY = 0x8,
        /// Pixel shader stage.
        const STAGE_PIXEL    = 0x10,
        /// Compute shader stage.
        const STAGE_COMPUTE  = 0x20,
        /// All graphics pipeline shader stages.
        const STAGE_GRAPHICS = STAGE_VERTEX.bits | STAGE_HULL.bits |
            STAGE_DOMAIN.bits | STAGE_GEOMETRY.bits | STAGE_PIXEL.bits,
        /// All shader stages.
        const STAGE_ALL      = STAGE_GRAPHICS.bits | STAGE_COMPUTE.bits,
    }
);

/// Which program stage this shader represents.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Stage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Pixel,
    Compute
}

/// An error type for creating shaders.
#[derive(Clone, Debug, PartialEq)]
pub enum CreateShaderError {
    /// The device does not support the requested shader model.
    ModelNotSupported,
    /// The device does not support the shader stage.
    StageNotSupported(Stage),
    /// The shader failed to compile.
    CompilationFailed(String),
}

impl fmt::Display for CreateShaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let desc = self.description();
        match *self {
            CreateShaderError::StageNotSupported(ref stage) => write!(f, "{}: {:?}", desc, stage),
            CreateShaderError::CompilationFailed(ref string) => write!(f, "{}: {}", desc, string),
            _ => write!(f, "{}", desc),
        }
    }
}

impl Error for CreateShaderError {
    fn description(&self) -> &str {
        match *self {
            CreateShaderError::ModelNotSupported => "The device does not support the requested shader model",
            CreateShaderError::StageNotSupported(_) => "The device does not support the shader stage",
            CreateShaderError::CompilationFailed(_) => "The shader failed to compile",
        }
    }
}
