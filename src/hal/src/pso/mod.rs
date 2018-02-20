//! Raw Pipeline State Objects
//!
//! This module contains items used to create and manage a raw pipeline state object. Most users
//! will want to use the typed and safe `PipelineState`. See the `pso` module inside the `gfx`
//! crate.

use {device, pass};
use std::error::Error;
use std::fmt;

mod compute;
mod descriptor;
mod graphics;
mod input_assembler;
mod output_merger;

pub use self::compute::*;
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
    /// Shader compilation error.
    Shader(device::ShaderError),
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreationError::InvalidSubpass(id) => write!(f, "{}: {:?}", self.description(), id),
            CreationError::Shader(ref err) => write!(f, "{}: {:?}", self.description(), err),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        match *self {
            CreationError::Other => "Unknown other error.",
            CreationError::InvalidSubpass(_) => "Invalid subpass index.",
            CreationError::Shader(_) => "Shader compilation error.",
        }
    }
}

bitflags!(
    /// Stages of the logical pipeline.
    ///
    /// The pipeline is structured by the ordering of the flags.
    /// Some stages are queue type dependent.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct PipelineStage: u32 {
        /// Beginning of the command queue.
        const TOP_OF_PIPE = 0x1;
        /// Indirect data consumption.
        const DRAW_INDIRECT = 0x2;
        /// Vertex data consumption.
        const VERTEX_INPUT = 0x4;
        /// Vertex shader execution.
        const VERTEX_SHADER = 0x8;
        /// Hull shader execution.
        const HULL_SHADER = 0x10;
        /// Domain shader execution.
        const DOMAIN_SHADER = 0x20;
        /// Geometry shader execution.
        const GEOMETRY_SHADER = 0x40;
        /// Fragment shader execution.
        const FRAGMENT_SHADER = 0x80;
        /// Stage of early depth and stencil test.
        const EARLY_FRAGMENT_TESTS = 0x100;
        /// Stage of late depth and stencil test.
        const LATE_FRAGMENT_TESTS = 0x200;
        /// Stage of final color value calculation.
        const COLOR_ATTACHMENT_OUTPUT = 0x400;
        /// Compute shader execution,
        const COMPUTE_SHADER = 0x800;
        /// Copy/Transfer command execution.
        const TRANSFER = 0x1000;
        /// End of the command queue.
        const BOTTOM_OF_PIPE = 0x2000;
        /// Read/Write access from host.
        /// (Not a real pipeline stage)
        const HOST = 0x4000;
    }
);

bitflags!(
    /// Combination of different shader pipeline stages.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ShaderStageFlags: u16 {
        /// Vertex shader stage.
        const VERTEX   = 0x1;
        /// Hull (tessellation) shader stage.
        const HULL     = 0x2;
        /// Domain (tessellation) shader stage.
        const DOMAIN   = 0x4;
        /// Geometry shader stage.
        const GEOMETRY = 0x8;
        /// Fragment shader stage.
        const FRAGMENT = 0x10;
        /// Compute shader stage.
        const COMPUTE  = 0x20;
        /// All graphics pipeline shader stages.
        const GRAPHICS = Self::VERTEX.bits | Self::HULL.bits |
            Self::DOMAIN.bits | Self::GEOMETRY.bits | Self::FRAGMENT.bits;
        /// All shader stages.
        const ALL      = Self::GRAPHICS.bits | Self::COMPUTE.bits;
    }
);

// Note: this type is only needed for backends, not used anywhere within gfx_hal.
/// Which program stage this shader represents.
/// DOC TODO
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Stage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Fragment,
    Compute
}

impl From<Stage> for ShaderStageFlags {
    fn from(stage: Stage) -> Self {
        match stage {
            Stage::Vertex => ShaderStageFlags::VERTEX,
            Stage::Hull => ShaderStageFlags::HULL,
            Stage::Domain => ShaderStageFlags::DOMAIN,
            Stage::Geometry => ShaderStageFlags::GEOMETRY,
            Stage::Fragment => ShaderStageFlags::FRAGMENT,
            Stage::Compute => ShaderStageFlags::COMPUTE,
        }
    }
}


/// Shader entry point.
#[derive(Debug, Copy)]
pub struct EntryPoint<'a, B: Backend> {
    /// Entry point name.
    pub entry: &'a str,
    /// Shader module reference.
    pub module: &'a B::ShaderModule,
    /// Specialization info.
    pub specialization: &'a [Specialization],
}

impl<'a, B: Backend> Clone for EntryPoint<'a, B> {
    fn clone(&self) -> Self {
        EntryPoint {
            entry: self.entry,
            module: self.module,
            specialization: self.specialization,
        }
    }
}

bitflags!(
    /// Pipeline creation flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct PipelineCreationFlags: u32 {
        /// Disable pipeline optimizations.
        ///
        /// May speedup pipeline creation.
        const DISABLE_OPTIMIZATION = 0x1;
        /// Allow derivatives of the pipeline.
        ///
        /// Must be set when pipelines set the pipeline as base.
        const ALLOW_DERIVATIVES = 0x2;
    }
);

/// DOC TODO
#[derive(Debug)]
pub enum BasePipeline<'a, P: 'a> {
    /// Referencing an existing pipeline as parent.
    Pipeline(&'a P),
    /// A pipeline in the same create pipelines call.
    ///
    /// The index of the parent must be lower than the index of the child.
    Index(usize),
    /// DOC TODO
    None,
}

/// Specialization information for pipelines.
/// 
/// Specialization constants allow for easy configuration of 
/// multiple similar pipelines.  For example, there may be a 
/// boolean exposed to the shader that switches the specularity on/off
/// provided via a specialization constant.
/// That would produce separate PSO's for the "on" and "off" states 
/// but they share most of the internal stuff and are fast to produce. 
/// More importantly, they are fast to execute, since the driver 
/// can optimize out the branch on that other PSO creation.
#[derive(Debug, Clone)]
pub struct Specialization {
    /// Constant identifier in shader source.
    pub id: u32,
    /// Value to override specialization constant.
    pub value: Constant,
}

/// Scalar specialization constant with value for overriding.
#[derive(Debug, Clone)]
pub enum Constant {
    /// `bool` value.
    Bool(bool),
    /// `u32` value.
    U32(u32),
    /// `u64` value.
    U64(u64),
    /// `i32` value.
    I32(i32),
    /// `i64` value.
    I64(i64),
    /// `f32` value.
    F32(f32),
    /// `f64` value.
    F64(f64),
}
