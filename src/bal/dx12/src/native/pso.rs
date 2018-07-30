//! Pipeline state

use super::com::WeakPtr;
use super::Blob;
use std::ops::Deref;
use std::ptr;
use winapi::um::d3d12;

bitflags! {
    pub struct PipelineStateFlags: u32 {
        const TOOL_DEBUG = d3d12::D3D12_PIPELINE_STATE_FLAG_TOOL_DEBUG;
    }
}

#[derive(Copy, Clone)]
pub struct Shader(d3d12::D3D12_SHADER_BYTECODE);
impl Shader {
    pub fn null() -> Self {
        Shader(d3d12::D3D12_SHADER_BYTECODE {
            BytecodeLength: 0,
            pShaderBytecode: ptr::null(),
        })
    }

    // `blob` may not be null.
    pub fn from_blob(blob: Blob) -> Self {
        Shader(d3d12::D3D12_SHADER_BYTECODE {
            BytecodeLength: unsafe { blob.GetBufferSize() },
            pShaderBytecode: unsafe { blob.GetBufferPointer() },
        })
    }
}

impl Deref for Shader {
    type Target = d3d12::D3D12_SHADER_BYTECODE;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Option<Blob>> for Shader {
    fn from(blob: Option<Blob>) -> Self {
        match blob {
            Some(b) => Shader::from_blob(b),
            None => Shader::null(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct CachedPSO(d3d12::D3D12_CACHED_PIPELINE_STATE);
impl CachedPSO {
    pub fn null() -> Self {
        CachedPSO(d3d12::D3D12_CACHED_PIPELINE_STATE {
            CachedBlobSizeInBytes: 0,
            pCachedBlob: ptr::null(),
        })
    }

    // `blob` may not be null.
    pub fn from_blob(blob: Blob) -> Self {
        CachedPSO(d3d12::D3D12_CACHED_PIPELINE_STATE {
            CachedBlobSizeInBytes: unsafe { blob.GetBufferSize() },
            pCachedBlob: unsafe { blob.GetBufferPointer() },
        })
    }
}

impl Deref for CachedPSO {
    type Target = d3d12::D3D12_CACHED_PIPELINE_STATE;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type PipelineState = WeakPtr<d3d12::ID3D12PipelineState>;

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
