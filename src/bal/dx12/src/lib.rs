extern crate gfx_bal as bal;
extern crate winapi;
extern crate wio;
#[macro_use]
extern crate static_assertions;
#[macro_use]
extern crate bitflags;

use winapi::shared::dxgiformat;
use winapi::um::d3d12;
use winapi::um::d3dcommon;

pub use winapi::shared::winerror::HRESULT;

pub mod barrier;
pub mod copy;
pub mod native;
pub mod spirv;

pub type D3DResult<T> = (T, HRESULT);
pub type GpuAddress = d3d12::D3D12_GPU_VIRTUAL_ADDRESS;
pub type Format = dxgiformat::DXGI_FORMAT;
pub type Rect = d3d12::D3D12_RECT;
pub type NodeMask = u32;

pub type TextureAddressMode = [d3d12::D3D12_TEXTURE_ADDRESS_MODE; 3];

#[repr(u32)]
pub enum FeatureLevel {
    L9_1 = d3dcommon::D3D_FEATURE_LEVEL_9_1,
    L9_2 = d3dcommon::D3D_FEATURE_LEVEL_9_2,
    L9_3 = d3dcommon::D3D_FEATURE_LEVEL_9_3,
    L10_0 = d3dcommon::D3D_FEATURE_LEVEL_10_0,
    L10_1 = d3dcommon::D3D_FEATURE_LEVEL_10_1,
    L11_0 = d3dcommon::D3D_FEATURE_LEVEL_11_0,
    L11_1 = d3dcommon::D3D_FEATURE_LEVEL_11_1,
    L12_0 = d3dcommon::D3D_FEATURE_LEVEL_12_0,
    L12_1 = d3dcommon::D3D_FEATURE_LEVEL_12_1,
}
