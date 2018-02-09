//! Compute pipeline descriptor.

use Backend;
use super::{BasePipeline, EntryPoint, PipelineCreationFlags};

/// A description of the data needed to construct a compute pipeline.
#[derive(Debug)]
pub struct ComputePipelineDesc<'a, B: Backend> {
    /// DOC TODO
    pub shader: EntryPoint<'a, B>,
    /// Pipeline layout.
    pub layout: &'a B::PipelineLayout,
    /// DOC TODO
    pub flags: PipelineCreationFlags,
    /// DOC TODO
    pub parent: BasePipeline<'a, B::ComputePipeline>,
}

impl<'a, B: Backend> ComputePipelineDesc<'a, B> {
    /// Create a new empty PSO descriptor.
    pub fn new(
        shader: EntryPoint<'a, B>,
        layout: &'a B::PipelineLayout,
    ) -> Self {
        ComputePipelineDesc {
            shader,
            layout,
            flags: PipelineCreationFlags::empty(),
            parent: BasePipeline::None,
        }
    }
}
