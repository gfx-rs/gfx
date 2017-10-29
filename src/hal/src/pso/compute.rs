//! Compute pipeline descriptor.

use Backend;
use super::{BaseCompute, PipelineCreationFlags};

///
#[derive(Debug)]
pub struct ComputePipelineDesc<'a, B: Backend> {
    /// Pipeline layout.
    pub layout: &'a B::PipelineLayout,
    ///
    pub flags: PipelineCreationFlags,
    ///
    pub parent: BaseCompute<'a, B>,
}
