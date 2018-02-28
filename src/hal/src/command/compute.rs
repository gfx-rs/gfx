use std::borrow::Borrow;

use {Backend, WorkGroupCount};
use buffer::Offset;
use queue::capability::{Compute, Supports};
use super::{CommandBuffer, RawCommandBuffer, Shot, Level};

impl<'a, B: Backend, C: Supports<Compute>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    ///
    pub fn bind_compute_pipeline(&mut self, pipeline: &B::ComputePipeline) {
        self.raw.bind_compute_pipeline(pipeline)
    }

    ///
    pub fn bind_compute_descriptor_sets<T>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<B::DescriptorSet>,
    {
        self.raw.bind_compute_descriptor_sets(layout, first_set, sets)
    }

    ///
    pub fn dispatch(&mut self, count: WorkGroupCount) {
        self.raw.dispatch(count)
    }

    ///
    pub fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: Offset) {
        self.raw.dispatch_indirect(buffer, offset)
    }

    ///
    pub fn push_compute_constants(&mut self, layout: &B::PipelineLayout, offset: u32, constants: &[u32]) {
        self.raw.push_compute_constants(layout, offset, constants);
    }
}
