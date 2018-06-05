//! `CommandBuffer` methods for compute operations.

use std::borrow::Borrow;

use {Backend, WorkGroupCount};
use buffer::Offset;
use queue::capability::{Compute, Supports};
use super::{CommandBuffer, DescriptorSetOffset, RawCommandBuffer, Shot, Level};

impl<'a, B: Backend, C: Supports<Compute>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn bind_compute_pipeline(&mut self, pipeline: &B::ComputePipeline) {
        self.raw.bind_compute_pipeline(pipeline)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn bind_compute_descriptor_sets<I, J>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<B::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<DescriptorSetOffset>,
    {
        self.raw.bind_compute_descriptor_sets(layout, first_set, sets, offsets)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn dispatch(&mut self, count: WorkGroupCount) {
        self.raw.dispatch(count)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: Offset) {
        self.raw.dispatch_indirect(buffer, offset)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn push_compute_constants(&mut self, layout: &B::PipelineLayout, offset: u32, constants: &[u32]) {
        self.raw.push_compute_constants(layout, offset, constants);
    }
}
