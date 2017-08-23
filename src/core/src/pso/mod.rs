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

pub mod descriptor;
pub mod graphics;
pub mod input_assembler;
pub mod output_merger;

pub use self::descriptor::*;
pub use self::graphics::GraphicsPipelineDesc;
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
