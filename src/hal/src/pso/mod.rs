//! Raw Pipeline State Objects
//!
//! This module contains items used to create and manage Pipelines.

use {device, pass};
use std::error::Error;
use std::fmt;
use std::ops::Range;

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
    pub struct ShaderStageFlags: u32 {
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
#[allow(missing_docs)]
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

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Stage::Vertex => "vertex",
            Stage::Hull => "hull",
            Stage::Domain => "domain",
            Stage::Geometry => "geometry",
            Stage::Fragment => "fragment",
            Stage::Compute => "compute"
        })
    }
}


/// Shader entry point.
#[derive(Debug, Copy)]
pub struct EntryPoint<'a, B: Backend> {
    /// Entry point name.
    pub entry: &'a str,
    /// Shader module reference.
    pub module: &'a B::ShaderModule,
    /// Specialization.
    pub specialization: Specialization<'a>,
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
        /// Allow derivatives (children) of the pipeline.
        ///
        /// Must be set when pipelines set the pipeline as base.
        const ALLOW_DERIVATIVES = 0x2;
    }
);

/// A reference to a parent pipeline.  The assumption is that
/// a parent and derivative/child pipeline have most settings
/// in common, and one may be switched for another more quickly
/// than entirely unrelated pipelines would be.
#[derive(Debug)]
pub enum BasePipeline<'a, P: 'a> {
    /// Referencing an existing pipeline as parent.
    Pipeline(&'a P),
    /// A pipeline in the same create pipelines call.
    ///
    /// The index of the parent must be lower than the index of the child.
    Index(usize),
    /// No parent pipeline exists.
    None,
}

/// Specialization constant for pipelines.
///
/// Specialization constants allow for easy configuration of
/// multiple similar pipelines. For example, there may be a
/// boolean exposed to the shader that switches the specularity on/off
/// provided via a specialization constant.
/// That would produce separate PSO's for the "on" and "off" states
/// but they share most of the internal stuff and are fast to produce.
/// More importantly, they are fast to execute, since the driver
/// can optimize out the branch on that other PSO creation.
#[derive(Debug, Clone)]
pub struct SpecializationConstant {
    /// Constant identifier in shader source.
    pub id: u32,
    /// Value to override specialization constant.
    pub range: Range<u16>,
}

/// Specialization information structure.
#[derive(Debug, Copy)]
pub struct Specialization<'a> {
    /// Constant array.
    pub constants: &'a [SpecializationConstant],
    /// Raw data.
    pub data: &'a [u8],
}

impl<'a> Default for Specialization<'a> {
    fn default() -> Self {
        Specialization {
            constants: &[],
            data: &[],
        }
    }
}

impl<'a> Clone for Specialization<'a> {
    fn clone(&self) -> Self {
        Specialization {
            constants: self.constants,
            data: self.data,
        }
    }
}

/// Pipeline state which may be static or dynamic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum State<T> {
    /// Static state that cannot be altered.
    Static(T),
    /// Dynamic state set through a command buffer.
    Dynamic,
}

impl<T> State<T> {
    /// Returns the static value or a default.
    pub fn static_or(self, default: T) -> T {
        match self {
            State::Static(v) => v,
            State::Dynamic => default,
        }
    }

    /// Whether the state is static.
    pub fn is_static(self) -> bool {
        match self {
            State::Static(_) => true,
            State::Dynamic => false,
        }
    }

    /// Whether the state is dynamic.
    pub fn is_dynamic(self) -> bool {
        !self.is_static()
    }
}
