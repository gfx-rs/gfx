//! Combine mesh data with pipeline state.
//!
//! Suitable for use when PSO is always used with the same one mesh.

use { Resources, Slice, PipelineState, Encoder, CommandBuffer };
use super::PipelineData;

/// Mesh-PSO bundle.
pub struct Bundle<R: Resources, Data: PipelineData<R>> {
    /// Mesh slice
    pub slice: Slice<R>,
    /// Pipeline state
    pub pso: PipelineState<R, Data::Meta>,
    /// Pipeline data for mesh
    pub data: Data,
}

impl<R: Resources, Data: PipelineData<R>> Bundle<R, Data> {
    /// Create new Bundle
    pub fn new(slice: Slice<R>, pso: PipelineState<R, Data::Meta>, data: Data) -> Self
    {
        Bundle {
            slice: slice,
            pso: pso,
            data: data,
        }
    }

    /// Draw bundle using encoder.
    pub fn encode<C>(&self, encoder: &mut Encoder<R, C>) where
        C: CommandBuffer<R> {
        encoder.draw(&self.slice, &self.pso, &self.data);
    }
}
