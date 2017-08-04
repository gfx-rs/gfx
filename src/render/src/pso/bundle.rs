//! Combine slice data with pipeline state.
//!
//! Suitable for use when PSO is always used with the same one slice.

use {Backend, Slice, PipelineState, GraphicsEncoder};
use super::PipelineData;

/// Slice-PSO bundle.
pub struct Bundle<B: Backend, Data: PipelineData<B>> {
    /// Slice
    pub slice: Slice<B>,
    /// Pipeline state
    pub pso: PipelineState<B, Data::Meta>,
    /// Pipeline data
    pub data: Data,
}

impl<B: Backend, Data: PipelineData<B>> Bundle<B, Data> {
    /// Create new Bundle
    pub fn new(slice: Slice<B>, pso: PipelineState<B, Data::Meta>, data: Data) -> Self
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
