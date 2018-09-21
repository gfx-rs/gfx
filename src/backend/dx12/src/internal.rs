use hal::backend::FastHashMap;
use std::ffi::CStr;
use std::sync::Mutex;
use std::{mem, ptr};

use d3d12;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::{dxgiformat, dxgitype, winerror};
use winapi::um::d3d12::*;
use winapi::Interface;

use native::{self, descriptor, pso};

#[derive(Clone)]
pub struct BlitPipe {
    pub pipeline: native::PipelineState,
    pub signature: native::RootSignature,
}

impl BlitPipe {
    pub unsafe fn destroy(&self) {
        self.pipeline.destroy();
        self.signature.destroy();
    }
}

// Information to pass to the shader
#[repr(C)]
pub struct BlitData {
    pub src_offset: [f32; 2],
    pub src_extent: [f32; 2],
    pub layer: f32,
    pub level: f32,
}

pub type BlitKey = (dxgiformat::DXGI_FORMAT, d3d12::D3D12_FILTER);
type BlitMap = FastHashMap<BlitKey, BlitPipe>;

pub(crate) struct ServicePipes {
    pub(crate) device: native::Device,
    blits_2d_color: Mutex<BlitMap>,
}

impl ServicePipes {
    pub fn new(device: native::Device) -> Self {
        ServicePipes {
            device,
            blits_2d_color: Mutex::new(FastHashMap::default()),
        }
    }

    pub unsafe fn destroy(&self) {
        let blits = self.blits_2d_color.lock().unwrap();
        for (_, pipe) in &*blits {
            pipe.destroy();
        }
    }

    pub fn get_blit_2d_color(&self, key: BlitKey) -> BlitPipe {
        let mut blits = self.blits_2d_color.lock().unwrap();
        blits
            .entry(key)
            .or_insert_with(|| self.create_blit_2d_color(key))
            .clone()
    }

    fn create_blit_2d_color(&self, (dst_format, filter): BlitKey) -> BlitPipe {
        let descriptor_range = [descriptor::DescriptorRange::new(
            descriptor::DescriptorRangeType::SRV,
            1,
            0,
            0,
            0,
        )];

        let root_parameters = [
            descriptor::RootParameter::descriptor_table(
                descriptor::ShaderVisibility::All,
                &descriptor_range,
            ),
            descriptor::RootParameter::constants(
                descriptor::ShaderVisibility::All,
                0,
                0,
                (mem::size_of::<BlitData>() / 4) as _,
            ),
        ];

        let static_samplers = [descriptor::StaticSampler::new(
            descriptor::ShaderVisibility::PS,
            0,
            0,
            filter,
            [
                d3d12::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
                d3d12::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
                d3d12::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            ],
            0.0,
            0,
            d3d12::D3D12_COMPARISON_FUNC_ALWAYS,
            descriptor::StaticBorderColor::TransparentBlack,
            0.0..d3d12::D3D12_FLOAT32_MAX,
        )];

        let ((signature_raw, error), _hr) = native::RootSignature::serialize(
            descriptor::RootSignatureVersion::V1_0,
            &root_parameters,
            &static_samplers,
            descriptor::RootSignatureFlags::empty(),
        );

        if !error.is_null() {
            error!("D3D12SerializeRootSignature error: {:?}", unsafe {
                error.as_c_str().to_str().unwrap()
            });
            unsafe { error.destroy() };
        }

        let (signature, _hr) = self.device.create_root_signature(signature_raw, 0);
        unsafe { signature_raw.destroy() };

        let shader_src = include_bytes!("../shaders/blit.hlsl");
        // TODO: check results
        let ((vs, _), _hr_vs) = pso::Shader::compile(
            shader_src,
            unsafe { CStr::from_bytes_with_nul_unchecked(b"vs_5_0\0") },
            unsafe { CStr::from_bytes_with_nul_unchecked(b"vs_blit_2d\0") },
            pso::ShaderCompileFlags::empty(),
        );
        let ((ps, _), _hr_ps) = pso::Shader::compile(
            shader_src,
            unsafe { CStr::from_bytes_with_nul_unchecked(b"ps_5_0\0") },
            unsafe { CStr::from_bytes_with_nul_unchecked(b"ps_blit_2d\0") },
            pso::ShaderCompileFlags::empty(),
        );

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
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
        };
        let render_targets = [dummy_target; 8];

        let pso_desc = d3d12::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: signature.as_mut_ptr(),
            VS: *pso::Shader::from_blob(vs),
            PS: *pso::Shader::from_blob(ps),
            GS: *pso::Shader::null(),
            DS: *pso::Shader::null(),
            HS: *pso::Shader::null(),
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

        let mut pipeline = native::PipelineState::null();
        let hr = unsafe {
            self.device.CreateGraphicsPipelineState(
                &pso_desc,
                &d3d12::ID3D12PipelineState::uuidof(),
                pipeline.mut_void(),
            )
        };
        assert_eq!(hr, winerror::S_OK);

        BlitPipe {
            pipeline,
            signature,
        }
    }
}
