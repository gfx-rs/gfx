//! Combine slice data with pipeline state.
//!
//! Suitable for use when PSO is always used with the same one slice.

use {Backend, Resources, Slice, PipelineState, GraphicsEncoder, CommandBuffer };
use super::PipelineData;

/// Slice-PSO bundle.
pub struct Bundle<B: Backend, Data: PipelineData<B::Resources>> {
    /// Slice
    pub slice: Slice<B::Resources>,
    /// Pipeline state
    pub pso: PipelineState<B::Resources, Data::Meta>,
    /// Pipeline data
    pub data: Data,
}

impl<B: Backend, Data: PipelineData<B::Resources>> Bundle<B, Data> {
    /// Create new Bundle
    pub fn new(slice: Slice<B::Resources>, pso: PipelineState<B::Resources, Data::Meta>, data: Data) -> Self
    {
        Bundle {
            slice: slice,
            pso: pso,
            data: data,
        }
    }

    /// Draw bundle using encoder.
    pub fn encode(&self, encoder: &mut GraphicsEncoder<B>) {
        encoder.draw(&self.slice, &self.pso, &self.data);
    }
}
