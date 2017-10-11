
use core::pass::Attachment;
use core::{pso, texture};
use winapi;

use std::collections::BTreeMap;

#[derive(Debug, Hash)]
pub struct ShaderLib {
    pub shaders: BTreeMap<pso::EntryPoint, *mut winapi::ID3DBlob>,
}
unsafe impl Send for ShaderLib { }
unsafe impl Sync for ShaderLib { }

#[derive(Debug, Hash, Clone)]
pub struct RenderPass {
    pub attachments: Vec<Attachment>,
    pub subpasses: Vec<()>, // TODO
}

#[derive(Debug, Hash)]
pub struct GraphicsPipeline {
    pub raw: *mut winapi::ID3D12PipelineState,
    pub topology: winapi::D3D12_PRIMITIVE_TOPOLOGY,
}
unsafe impl Send for GraphicsPipeline { }
unsafe impl Sync for GraphicsPipeline { }

#[derive(Debug, Hash)]
pub struct ComputePipeline {
    pub raw: *mut winapi::ID3D12PipelineState,
}

unsafe impl Send for ComputePipeline { }
unsafe impl Sync for ComputePipeline { }

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub raw: *mut winapi::ID3D12RootSignature,
}
unsafe impl Send for PipelineLayout { }
unsafe impl Sync for PipelineLayout { }

#[derive(Debug, Hash, Clone)]
pub struct FrameBuffer {
    pub color: Vec<RenderTargetView>,
    pub depth_stencil: Vec<DepthStencilView>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Buffer {
    pub resource: *mut winapi::ID3D12Resource,
    pub size_in_bytes: u32,
    pub stride: u32,
}
unsafe impl Send for Buffer { }
unsafe impl Sync for Buffer { }

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Image {
    pub resource: *mut winapi::ID3D12Resource,
    pub kind: texture::Kind,
    pub dxgi_format: winapi::DXGI_FORMAT,
    pub bits_per_texel: u8,
}
unsafe impl Send for Image { }
unsafe impl Sync for Image { }

#[derive(Debug, Hash, Clone)]
pub struct RenderTargetView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug, Hash, Clone)]
pub struct DepthStencilView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    // pub bindings: Vec<d::DescriptorSetLayoutBinding>,
}
