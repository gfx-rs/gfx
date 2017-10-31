//! Compute pipeline descriptor.

use Backend;
use super::{BaseCompute, BasePipeline, EntryPoint, PipelineCreationFlags};

///
#[derive(Debug)]
pub struct ComputePipelineDesc<'a, B: Backend> {
    ///
    pub shader: EntryPoint<'a, B>,
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
