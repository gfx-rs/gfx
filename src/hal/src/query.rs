//! Query operations

use Backend;


///
pub type QueryId = u32;

///
#[derive(Debug)]
pub struct Query<'a, B: Backend> {
    ///
    pub pool: &'a B::QueryPool,
    ///
    pub id: QueryId,
}

bitflags!(
    /// Query control flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct QueryControl: u8 {
        /// Occlusion queries **must** return the exact sampler number.
        ///
        /// Requires `precise_occlusion_query` device feature.
        const PRECISE = 0x1;
    }
);

/// Type of queries in a query pool.
pub enum QueryType {
    /// Occlusion query.
    Occlusion,
    /// Pipeline statistic data.
    PipelineStatistics(PipelineStatistic),
    ///
    Timestamp,
}

bitflags!(
    /// Pipeline statistic flags
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct PipelineStatistic: u16 {
        ///
        const INPUT_ASSEMBLY_VERTICES = 0x1;
        ///
        const INPUT_ASSEMBLY_PRIMITIVES = 0x2;
        ///
        const VERTEX_SHADER_INVOCATIONS = 0x4;
        ///
        const GEOMETRY_SHADER_INVOCATIONS = 0x8;
        ///
        const GEOMETRY_SHADER_PRIMITIVES = 0x10;
        ///
        const CLIPPING_INVOCATIONS = 0x20;
        ///
        const CLIPPING_PRIMITIVES = 0x40;
        ///
        const FRAGMENT_SHADER_INVOCATIONS = 0x80;
        ///
        const HULL_SHADER_PATCHES = 0x100;
        ///
        const DOMAIN_SHADER_INVOCATIONS = 0x200;
        ///
        const COMPUTE_SHADER_INVOCATIONS = 0x400;
    }
);
