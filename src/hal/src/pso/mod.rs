//! Raw Pipeline State Objects
//!
//! This module contains items used to create and manage a raw pipeline state object. Most users
//! will want to use the typed and safe `PipelineState`. See the `pso` module inside the `gfx`
//! crate.

use pass;
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

use Backend;

/// Error types happening upon PSO creation on the device side.
#[derive(Clone, Debug, PartialEq)]
pub enum CreationError {
    /// Unknown other error.
    Other,
    /// Invalid subpass (not part of renderpass).
    InvalidSubpass(pass::SubpassId),
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreationError::InvalidSubpass(id) => write!(f, "{}: {:?}", self.description(), id),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        match *self {
            CreationError::Other => "Unknown other error.",
            CreationError::InvalidSubpass(_) => "Invalid subpass index.",
        }
    }
}

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
        /// Fragment shader execution.
        const FRAGMENT_SHADER = 0x80,
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
        /// Fragment shader stage.
        const STAGE_FRAGMENT = 0x10,
        /// Compute shader stage.
        const STAGE_COMPUTE  = 0x20,
        /// All graphics pipeline shader stages.
        const STAGE_GRAPHICS = STAGE_VERTEX.bits | STAGE_HULL.bits |
            STAGE_DOMAIN.bits | STAGE_GEOMETRY.bits | STAGE_FRAGMENT.bits,
        /// All shader stages.
        const STAGE_ALL      = STAGE_GRAPHICS.bits | STAGE_COMPUTE.bits,
    }
);

//Note: this type is only needed for backends, not used anywhere within gfx_core.
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
    Fragment,
    Compute
}

/// Shader entry point.
#[derive(Debug)]
pub struct EntryPoint<'a, B: Backend> {
    /// Entry point name.
    pub entry: &'a str,
    /// Shader module reference.
    pub module: &'a B::ShaderModule,
}

impl<'a, B: Backend> Clone for EntryPoint<'a, B> {
    fn clone(&self) -> Self {
        EntryPoint {
            entry: self.entry,
            module: self.module,
        }
    }
}

impl<'a, B: Backend> PartialEq for EntryPoint<'a, B> {
    fn eq(&self, other: &Self) -> bool {
        self.entry.as_ptr() == other.entry.as_ptr() &&
        self.module as *const _ == other.module as *const _
    }
}

impl<'a, B: Backend> Copy for EntryPoint<'a, B> {}
impl<'a, B: Backend> Eq for EntryPoint<'a, B> {}
