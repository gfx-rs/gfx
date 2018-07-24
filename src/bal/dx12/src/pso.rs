//! Pipeline state

use winapi::um::d3d12;

#[repr(u32)]
pub enum Subobject {
    RootSignature = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_ROOT_SIGNATURE,
    VS = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS,
    PS = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS,
    DS = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DS,
    HS = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_HS,
    GS = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_GS,
    CS = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CS,
    StreamOutput = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT,
    Blend = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_BLEND,
    SampleMask = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK,
    Rasterizer = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER,
    DepthStencil = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL,
    InputLayout = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT,
    IBStripCut = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE,
    PrimitiveTopology = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY,
    RTFormats = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS,
    DSFormat = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT,
    SampleDesc = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_DESC,
    NodeMask = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_NODE_MASK,
    CachedPSO = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CACHED_PSO,
    Flags = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_FLAGS,
    DepthStencil1 = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1,
    // ViewInstancing = d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING,
}

/// Subobject of a pipeline stream description
#[repr(C)]
pub struct PipelineStateSubobject<T> {
    subobject_align: [usize; 0], // Subobjects must have the same alignment as pointers.
    subobject_type: d3d12::D3D12_PIPELINE_STATE_SUBOBJECT_TYPE,
    subobject: T,
}

impl<T> PipelineStateSubobject<T> {
    pub fn new(subobject_type: Subobject, subobject: T) -> Self {
        PipelineStateSubobject {
            subobject_align: [],
            subobject_type: subobject_type as _,
            subobject,
        }
    }
}
