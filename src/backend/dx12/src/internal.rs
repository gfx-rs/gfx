use hal::pso;
use spirv_cross::hlsl;
use std::{mem, ptr};
use std::collections::HashMap;
use std::sync::Mutex;

use d3d12;
use winapi::shared::{dxgiformat, dxgitype, winerror};
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::um::d3d12::*;
use wio::com::ComPtr;

use {device};

#[derive(Clone)]
pub struct Blit {
    pipeline: ComPtr<d3d12::ID3D12PipelineState>,
    signature: ComPtr<d3d12::ID3D12RootSignature>,
}

pub struct ServicePipes {
    device: ComPtr<d3d12::ID3D12Device>,
    blits_2d_color: Mutex<HashMap<(dxgiformat::DXGI_FORMAT, d3d12::D3D12_FILTER), Blit>>,
}

impl ServicePipes {
    pub fn new(device: ComPtr<d3d12::ID3D12Device>) -> Self {
        ServicePipes {
            device,
            blits_2d_color: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_blit_2d_color(&self, dst_format: dxgiformat::DXGI_FORMAT, filter: d3d12::D3D12_FILTER) -> Blit {
        let mut blits = self.blits_2d_color.lock().unwrap();
        blits
            .entry((dst_format, filter))
            .or_insert_with(|| self.create_blit_2d_color(dst_format, filter))
            .clone()
    }

    fn create_blit_2d_color(&self, dst_format: dxgiformat::DXGI_FORMAT, filter: d3d12::D3D12_FILTER) -> Blit {
        let descriptor_range = d3d12::D3D12_DESCRIPTOR_RANGE {
            RangeType: d3d12::D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 1,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            OffsetInDescriptorsFromTableStart: 0,
        };

        let mut root_parameters = [
            d3d12::D3D12_ROOT_PARAMETER {
                ParameterType: d3d12::D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                ShaderVisibility: d3d12::D3D12_SHADER_VISIBILITY_ALL,
                .. unsafe { mem::zeroed() }
            },
            d3d12::D3D12_ROOT_PARAMETER {
                ParameterType: d3d12::D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
                ShaderVisibility: d3d12::D3D12_SHADER_VISIBILITY_VERTEX,
                .. unsafe { mem::zeroed() }
            },
        ];

        *unsafe { root_parameters[0].u.DescriptorTable_mut() } = d3d12::D3D12_ROOT_DESCRIPTOR_TABLE {
            NumDescriptorRanges: 1,
            pDescriptorRanges: &descriptor_range,
        };

        *unsafe { root_parameters[1].u.Constants_mut() } = d3d12::D3D12_ROOT_CONSTANTS {
            ShaderRegister: 0,
            RegisterSpace: 0,
            Num32BitValues: 5,
        };

        let static_samplers = d3d12::D3D12_STATIC_SAMPLER_DESC {
            Filter: filter,
            AddressU: d3d12::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressV: d3d12::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressW: d3d12::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 0,
            ComparisonFunc: d3d12::D3D12_COMPARISON_FUNC_ALWAYS,
            BorderColor: d3d12::D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
            MinLOD: 0.0,
            MaxLOD: d3d12::D3D12_FLOAT32_MAX,
            ShaderRegister: 0,
            RegisterSpace: 0,
            ShaderVisibility: d3d12::D3D12_SHADER_VISIBILITY_PIXEL,
        };

        let root_signature_desc = d3d12::D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: root_parameters.len() as _,
            pParameters: root_parameters.as_ptr(),
            NumStaticSamplers: 1,
            pStaticSamplers: &static_samplers,
            Flags: d3d12::D3D12_ROOT_SIGNATURE_FLAG_NONE,
        };

        let mut signature = ptr::null_mut();
        let mut signature_raw = ptr::null_mut();
        let mut error = ptr::null_mut();

        // TODO: error handling
        unsafe {
            let _hr = d3d12::D3D12SerializeRootSignature(
                &root_signature_desc,
                d3d12::D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature_raw,
                &mut error,
            );

            if !error.is_null() {
                // TODO
                let error_output = (*error).GetBufferPointer();
                let message = ::std::ffi::CStr::from_ptr(error_output as *const _ as *const _);
                error!("D3D12SerializeRootSignature error: {:?}", message.to_str().unwrap());
                (*error).Release();
            }

            self.device.CreateRootSignature(
                0,
                (*signature_raw).GetBufferPointer(),
                (*signature_raw).GetBufferSize(),
                &d3d12::IID_ID3D12RootSignature,
                &mut signature as *mut *mut _ as *mut *mut _,
            );
            (*signature_raw).Release();
        }

        let shader_src = include_bytes!("../shaders/blit.hlsl");
        let vs = device::compile_shader(pso::Stage::Vertex, hlsl::ShaderModel::V5_0, "vs_blit_2d", shader_src).unwrap();
        let ps = device::compile_shader(pso::Stage::Fragment, hlsl::ShaderModel::V5_0, "ps_blit_2d", shader_src).unwrap();

        let mut rtvs = [dxgiformat::DXGI_FORMAT_UNKNOWN; 8];
        rtvs[0] = dst_format;

        let dummy_target = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: FALSE,
            LogicOpEnable: FALSE,
            SrcBlend: D3D12_BLEND_ZERO,
            DestBlend: D3D12_BLEND_ZERO,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ZERO,
            DestBlendAlpha: D3D12_BLEND_ZERO,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_CLEAR,
            RenderTargetWriteMask: 0,
        };
        let render_targets = [dummy_target; 8];

        let pso_desc = d3d12::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: signature,
            VS: device::shader_bytecode(vs),
            PS: device::shader_bytecode(ps),
            GS: device::shader_bytecode(ptr::null_mut()),
            DS: device::shader_bytecode(ptr::null_mut()),
            HS: device::shader_bytecode(ptr::null_mut()),
            StreamOutput: d3d12::D3D12_STREAM_OUTPUT_DESC {
                pSODeclaration: ptr::null(),
                NumEntries: 0,
                pBufferStrides: ptr::null(),
                NumStrides: 0,
                RasterizedStream: 0,
            },
            BlendState: d3d12::D3D12_BLEND_DESC {
                AlphaToCoverageEnable: FALSE,
                IndependentBlendEnable: FALSE,
                RenderTarget: render_targets,
            },
            SampleMask: !0,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                FrontCounterClockwise: TRUE,
                DepthBias: 0,
                DepthBiasClamp: 0.0,
                SlopeScaledDepthBias: 0.0,
                DepthClipEnable: FALSE,
                MultisampleEnable: FALSE,
                ForcedSampleCount: 0,
                AntialiasedLineEnable: FALSE,
                ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
            },
            DepthStencilState: unsafe { mem::zeroed() },
            InputLayout: d3d12::D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: ptr::null(),
                NumElements: 0,
            },
            IBStripCutValue: d3d12::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: rtvs,
            DSVFormat: dxgiformat::DXGI_FORMAT_UNKNOWN,
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            NodeMask: 0,
            CachedPSO: d3d12::D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: d3d12::D3D12_PIPELINE_STATE_FLAG_NONE,
        };

        let mut pipeline = ptr::null_mut();
        let hr = unsafe {
            self.device.CreateGraphicsPipelineState(
                &pso_desc,
                &d3d12::IID_ID3D12PipelineState,
                &mut pipeline as *mut *mut _ as *mut *mut _)
        };
        assert_eq!(hr, winerror::S_OK);

        Blit {
            pipeline: unsafe { ComPtr::from_raw(pipeline) },
            signature: unsafe { ComPtr::from_raw(signature) },
        }
    }
}