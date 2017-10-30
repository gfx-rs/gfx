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

impl<'a, B: Backend> ComputePipelineDesc<'a, B> {
    /// Create a new empty PSO descriptor.
    pub fn new(
        layout: &'a B::PipelineLayout,
    ) -> Self {
        ComputePipelineDesc {
            layout,
            flags: PipelineCreationFlags::empty(),
            parent: BaseCompute::None,
        }
    }
}
