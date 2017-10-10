use Backend;
use queue::capability::{Compute, Supports};
use super::{CommandBuffer, RawCommandBuffer};


impl<'a, B: Backend, C: Supports<Compute>> CommandBuffer<'a, B, C> {
    ///
    pub fn bind_compute_pipeline(&mut self, pipeline: &B::ComputePipeline) {
        self.raw.bind_compute_pipeline(pipeline)
    }

    ///
    pub fn bind_compute_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    ) {
        self.raw.bind_compute_descriptor_sets(layout, first_set, sets)
    }

    ///
    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.raw.dispatch(x, y, z)
    }

    ///
    pub fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: u64) {
        self.raw.dispatch_indirect(buffer, offset)
    }
}
