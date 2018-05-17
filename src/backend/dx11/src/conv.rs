use hal::format::{Format};

use winapi::um::d3d11::*;
use winapi::um::d3dcommon::*;
use winapi::shared::dxgiformat::*;

// TODO: stolen from d3d12 backend, maybe share function somehow?
pub(crate) fn map_format(format: Format) -> Option<DXGI_FORMAT> {
    use hal::format::Format::*;

    let format = match format {
        R5g6b5Unorm => DXGI_FORMAT_B5G6R5_UNORM,
        R5g5b5a1Unorm => DXGI_FORMAT_B5G5R5A1_UNORM,
        R8Unorm => DXGI_FORMAT_R8_UNORM,
        R8Inorm => DXGI_FORMAT_R8_SNORM,
        R8Uint => DXGI_FORMAT_R8_UINT,
        R8Int => DXGI_FORMAT_R8_SINT,
        Rg8Unorm => DXGI_FORMAT_R8G8_UNORM,
        Rg8Inorm => DXGI_FORMAT_R8G8_SNORM,
        Rg8Uint => DXGI_FORMAT_R8G8_UINT,
        Rg8Int => DXGI_FORMAT_R8G8_SINT,
        Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        Rgba8Inorm => DXGI_FORMAT_R8G8B8A8_SNORM,
        Rgba8Uint => DXGI_FORMAT_R8G8B8A8_UINT,
        Rgba8Int => DXGI_FORMAT_R8G8B8A8_SINT,
        Rgba8Srgb => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        Bgra8Srgb => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        A2b10g10r10Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
        A2b10g10r10Uint => DXGI_FORMAT_R10G10B10A2_UINT,
        R16Unorm => DXGI_FORMAT_R16_UNORM,
        R16Inorm => DXGI_FORMAT_R16_SNORM,
        R16Uint => DXGI_FORMAT_R16_UINT,
        R16Int => DXGI_FORMAT_R16_SINT,
        R16Float => DXGI_FORMAT_R16_FLOAT,
        Rg16Unorm => DXGI_FORMAT_R16G16_UNORM,
        Rg16Inorm => DXGI_FORMAT_R16G16_SNORM,
        Rg16Uint => DXGI_FORMAT_R16G16_UINT,
        Rg16Int => DXGI_FORMAT_R16G16_SINT,
        Rg16Float => DXGI_FORMAT_R16G16_FLOAT,
        Rgba16Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
        Rgba16Inorm => DXGI_FORMAT_R16G16B16A16_SNORM,
        Rgba16Uint => DXGI_FORMAT_R16G16B16A16_UINT,
        Rgba16Int => DXGI_FORMAT_R16G16B16A16_SINT,
        Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
        R32Uint => DXGI_FORMAT_R32_UINT,
        R32Int => DXGI_FORMAT_R32_SINT,
        R32Float => DXGI_FORMAT_R32_FLOAT,
        Rg32Uint => DXGI_FORMAT_R32G32_UINT,
        Rg32Int => DXGI_FORMAT_R32G32_SINT,
        Rg32Float => DXGI_FORMAT_R32G32_FLOAT,
        Rgb32Uint => DXGI_FORMAT_R32G32B32_UINT,
        Rgb32Int => DXGI_FORMAT_R32G32B32_SINT,
        Rgb32Float => DXGI_FORMAT_R32G32B32_FLOAT,
        Rgba32Uint => DXGI_FORMAT_R32G32B32A32_UINT,
        Rgba32Int => DXGI_FORMAT_R32G32B32A32_SINT,
        Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
        B10g11r11Ufloat => DXGI_FORMAT_R11G11B10_FLOAT,
        E5b9g9r9Ufloat => DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
        D16Unorm => DXGI_FORMAT_D16_UNORM,
        D32Float => DXGI_FORMAT_D32_FLOAT,
        D32FloatS8Uint => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        Bc1RgbUnorm => DXGI_FORMAT_BC1_UNORM,
        Bc1RgbSrgb => DXGI_FORMAT_BC1_UNORM_SRGB,
        Bc2Unorm => DXGI_FORMAT_BC2_UNORM,
        Bc2Srgb => DXGI_FORMAT_BC2_UNORM_SRGB,
        Bc3Unorm => DXGI_FORMAT_BC3_UNORM,
        Bc3Srgb => DXGI_FORMAT_BC3_UNORM_SRGB,
        Bc4Unorm => DXGI_FORMAT_BC4_UNORM,
        Bc4Inorm => DXGI_FORMAT_BC4_SNORM,
        Bc5Unorm => DXGI_FORMAT_BC5_UNORM,
        Bc5Inorm => DXGI_FORMAT_BC5_SNORM,
        Bc6hUfloat => DXGI_FORMAT_BC6H_UF16,
        Bc6hFloat => DXGI_FORMAT_BC6H_SF16,
        Bc7Unorm => DXGI_FORMAT_BC7_UNORM,
        Bc7Srgb => DXGI_FORMAT_BC7_UNORM_SRGB,

        _ => return None,
    };

    Some(format)
}
