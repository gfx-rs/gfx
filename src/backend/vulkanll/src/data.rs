// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ash::vk;
use core::{buffer, image, shade};
use core::command::ClearColor;
use core::factory::DescriptorType;
use core::format::{SurfaceType, ChannelType};
use core::image::{FilterMethod, PackedColor, WrapMode};
use core::memory::{self, ImageAccess, ImageLayout};
use core::pass::{AttachmentLoadOp, AttachmentStoreOp, AttachmentLayout};
use core::pso::{self, PipelineStage};
use core::IndexType;

pub fn map_format(surface: SurfaceType, chan: ChannelType) -> Option<vk::Format> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;
    Some(match surface {
        R4_G4 => match chan {
            Unorm => vk::Format::R4g4UnormPack8,
            _ => return None,
        },
        R4_G4_B4_A4 => match chan {
            Unorm => vk::Format::R4g4b4a4UnormPack16,
            _ => return None,
        },
        R5_G5_B5_A1 => match chan {
            Unorm => vk::Format::R5g5b5a1UnormPack16,
             _ => return None,
        },
        R5_G6_B5 => match chan {
            Unorm => vk::Format::R5g6b5UnormPack16,
             _ => return None,
        },
        R8 => match chan {
            Int   => vk::Format::R8Sint,
            Uint  => vk::Format::R8Uint,
            Inorm => vk::Format::R8Snorm,
            Unorm => vk::Format::R8Unorm,
            Srgb  => vk::Format::R8Srgb,
            _ => return None,
        },
        R8_G8 => match chan {
            Int   => vk::Format::R8g8Sint,
            Uint  => vk::Format::R8g8Uint,
            Inorm => vk::Format::R8g8Snorm,
            Unorm => vk::Format::R8g8Unorm,
            Srgb  => vk::Format::R8g8Srgb,
            _ => return None,
        },
        R8_G8_B8_A8 => match chan {
            Int   => vk::Format::R8g8b8a8Sint,
            Uint  => vk::Format::R8g8b8a8Uint,
            Inorm => vk::Format::R8g8b8a8Snorm,
            Unorm => vk::Format::R8g8b8a8Unorm,
            Srgb  => vk::Format::R8g8b8a8Srgb,
            _ => return None,
        },
        R10_G10_B10_A2 => match chan {
            Int   => vk::Format::A2r10g10b10SintPack32,
            Uint  => vk::Format::A2r10g10b10UintPack32,
            Inorm => vk::Format::A2r10g10b10SnormPack32,
            Unorm => vk::Format::A2r10g10b10UnormPack32,
            _ => return None,
        },
        R11_G11_B10 => match chan {
            Float => vk::Format::B10g11r11UfloatPack32,
            _ => return None,
        },
        R16 => match chan {
            Int   => vk::Format::R16Sint,
            Uint  => vk::Format::R16Uint,
            Inorm => vk::Format::R16Snorm,
            Unorm => vk::Format::R16Unorm,
            Float => vk::Format::R16Sfloat,
            _ => return None,
        },
        R16_G16 => match chan {
            Int   => vk::Format::R16g16Sint,
            Uint  => vk::Format::R16g16Uint,
            Inorm => vk::Format::R16g16Snorm,
            Unorm => vk::Format::R16g16Unorm,
            Float => vk::Format::R16g16Sfloat,
            _ => return None,
        },
        R16_G16_B16 => match chan {
            Int   => vk::Format::R16g16b16Sint,
            Uint  => vk::Format::R16g16b16Uint,
            Inorm => vk::Format::R16g16b16Snorm,
            Unorm => vk::Format::R16g16b16Unorm,
            Float => vk::Format::R16g16b16Sfloat,
            _ => return None,
        },
        R16_G16_B16_A16 => match chan {
            Int   => vk::Format::R16g16b16a16Sint,
            Uint  => vk::Format::R16g16b16a16Uint,
            Inorm => vk::Format::R16g16b16a16Snorm,
            Unorm => vk::Format::R16g16b16a16Unorm,
            Float => vk::Format::R16g16b16a16Sfloat,
            _ => return None,
        },
        R32 => match chan {
            Int   => vk::Format::R32Sint,
            Uint  => vk::Format::R32Uint,
            Float => vk::Format::R32Sfloat,
            _ => return None,
        },
        R32_G32 => match chan {
            Int   => vk::Format::R32g32Sint,
            Uint  => vk::Format::R32g32Uint,
            Float => vk::Format::R32g32Sfloat,
            _ => return None,
        },
        R32_G32_B32 => match chan {
            Int   => vk::Format::R32g32b32Sint,
            Uint  => vk::Format::R32g32b32Uint,
            Float => vk::Format::R32g32b32Sfloat,
            _ => return None,
        },
        R32_G32_B32_A32 => match chan {
            Int   => vk::Format::R32g32b32a32Sint,
            Uint  => vk::Format::R32g32b32a32Uint,
            Float => vk::Format::R32g32b32a32Sfloat,
            _ => return None,
        },
        B8_G8_R8_A8 => match chan {
            Unorm => vk::Format::B8g8r8a8Unorm,
            _ => return None,
        },
        D16 => match chan {
            Unorm  => vk::Format::D16Unorm,
            _ => return None,
        },
        D24 => match chan {
            Unorm => vk::Format::X8D24UnormPack32,
            _ => return None,
        },
        D24_S8 => match chan {
            Unorm => vk::Format::D24UnormS8Uint,
            _ => return None,
        },
        D32 => match chan {
            Float => vk::Format::D32Sfloat,
            _ => return None,
        },
    })
}

pub fn map_clear_color(value: ClearColor) -> vk::ClearColorValue {
    match value {
        ClearColor::Float(v) => vk::ClearColorValue::new_float32(v),
        ClearColor::Int(v)   => vk::ClearColorValue::new_int32(v),
        ClearColor::Uint(v)  => vk::ClearColorValue::new_uint32(v),
    }
}

pub fn map_attachment_load_op(op: AttachmentLoadOp) -> vk::AttachmentLoadOp {
    match op {
        AttachmentLoadOp::Load => vk::AttachmentLoadOp::Load,
        AttachmentLoadOp::Clear => vk::AttachmentLoadOp::Clear,
        AttachmentLoadOp::DontCare => vk::AttachmentLoadOp::DontCare,
    }
}

pub fn map_attachment_store_op(op: AttachmentStoreOp) -> vk::AttachmentStoreOp {
    match op {
        AttachmentStoreOp::Store => vk::AttachmentStoreOp::Store,
        AttachmentStoreOp::DontCare => vk::AttachmentStoreOp::DontCare,
    }
}

pub fn map_image_layout(layout: ImageLayout) -> vk::ImageLayout {
    match layout {
        ImageLayout::General => vk::ImageLayout::General,
        ImageLayout::ColorAttachmentOptimal => vk::ImageLayout::ColorAttachmentOptimal,
        ImageLayout::DepthStencilAttachmentOptimal => vk::ImageLayout::DepthStencilAttachmentOptimal,
        ImageLayout::DepthStencilReadOnlyOptimal => vk::ImageLayout::DepthStencilReadOnlyOptimal,
        ImageLayout::ShaderReadOnlyOptimal => vk::ImageLayout::ShaderReadOnlyOptimal,
        ImageLayout::TransferSrcOptimal => vk::ImageLayout::TransferSrcOptimal,
        ImageLayout::TransferDstOptimal => vk::ImageLayout::TransferDstOptimal,
        ImageLayout::Undefined => vk::ImageLayout::Undefined,
        ImageLayout::Preinitialized => vk::ImageLayout::Preinitialized,
        ImageLayout::Present => vk::ImageLayout::PresentSrcKhr,
    }
}

pub fn map_image_access(access: ImageAccess) -> vk::AccessFlags {
    let mut flags = vk::AccessFlags::empty();

    if access.contains(memory::RENDER_TARGET_CLEAR) {
        unimplemented!()
    }
    if access.contains(memory::RESOLVE_SRC) {
        unimplemented!()
    }
    if access.contains(memory::RESOLVE_DST) {
        unimplemented!()
    }
    if access.contains(memory::COLOR_ATTACHMENT_READ) {
        flags |= vk::ACCESS_COLOR_ATTACHMENT_READ_BIT;
    }
    if access.contains(memory::COLOR_ATTACHMENT_WRITE) {
        flags |= vk::ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
    }
    if access.contains(memory::TRANSFER_READ) {
        flags |= vk::ACCESS_TRANSFER_READ_BIT;
    }
    if access.contains(memory::TRANSFER_WRITE) {
        flags |= vk::ACCESS_TRANSFER_WRITE_BIT;
    }
    if access.contains(memory::SHADER_READ) {
        flags |= vk::ACCESS_SHADER_READ_BIT;
    }

    flags
}

pub fn map_pipeline_stage(stage: PipelineStage) -> vk::PipelineStageFlags {
    let mut flags = vk::PipelineStageFlags::empty();

    if stage.contains(pso::TOP_OF_PIPE) {
        flags |= vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT;
    }
    if stage.contains(pso::DRAW_INDIRECT) {
        flags |= vk::PIPELINE_STAGE_DRAW_INDIRECT_BIT;
    }
    if stage.contains(pso::VERTEX_INPUT) {
        flags |= vk::PIPELINE_STAGE_VERTEX_INPUT_BIT;
    }
    if stage.contains(pso::VERTEX_SHADER) {
        flags |= vk::PIPELINE_STAGE_VERTEX_SHADER_BIT;
    }
    if stage.contains(pso::HULL_SHADER) {
        flags |= vk::PIPELINE_STAGE_TESSELLATION_CONTROL_SHADER_BIT;
    }
    if stage.contains(pso::DOMAIN_SHADER) {
        flags |= vk::PIPELINE_STAGE_TESSELLATION_EVALUATION_SHADER_BIT;
    }
    if stage.contains(pso::GEOMETRY_SHADER) {
        flags |= vk::PIPELINE_STAGE_GEOMETRY_SHADER_BIT;
    }
    if stage.contains(pso::PIXEL_SHADER) {
        flags |= vk::PIPELINE_STAGE_FRAGMENT_SHADER_BIT;
    }
    if stage.contains(pso::EARLY_FRAGMENT_TESTS) {
        flags |= vk::PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT;
    }
    if stage.contains(pso::LATE_FRAGMENT_TESTS) {
        flags |= vk::PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT;
    }
    if stage.contains(pso::COLOR_ATTACHMENT_OUTPUT) {
        flags |= vk::PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
    }
    if stage.contains(pso::COMPUTE_SHADER) {
        flags |= vk::PIPELINE_STAGE_COMPUTE_SHADER_BIT;
    }
    if stage.contains(pso::TRANSFER) {
        flags |= vk::PIPELINE_STAGE_TRANSFER_BIT;
    }
    if stage.contains(pso::BOTTOM_OF_PIPE) {
        flags |= vk::PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT;
    }

    flags
}

pub fn map_buffer_usage(usage: buffer::Usage) -> vk::BufferUsageFlags {
    let mut flags = vk::BufferUsageFlags::empty();

    if usage.contains(buffer::TRANSFER_SRC) {
        flags |= vk::BUFFER_USAGE_TRANSFER_SRC_BIT;
    }
    if usage.contains(buffer::TRANSFER_DST) {
        flags |= vk::BUFFER_USAGE_TRANSFER_DST_BIT;
    }
    if usage.contains(buffer::CONSTANT) {
        flags |= vk::BUFFER_USAGE_UNIFORM_BUFFER_BIT;
    }
    if usage.contains(buffer::INDEX) {
        flags |= vk::BUFFER_USAGE_INDEX_BUFFER_BIT;
    }
    if usage.contains(buffer::INDIRECT) {
        flags |= vk::BUFFER_USAGE_INDIRECT_BUFFER_BIT;
    }
    if usage.contains(buffer::VERTEX) {
        flags |= vk::BUFFER_USAGE_VERTEX_BUFFER_BIT;
    }

    flags
}

pub fn map_image_usage(usage: image::Usage) -> vk::ImageUsageFlags {
    let mut flags = vk::ImageUsageFlags::empty();

    if usage.contains(image::TRANSFER_SRC) {
        flags |= vk::IMAGE_USAGE_TRANSFER_SRC_BIT;
    }
    if usage.contains(image::TRANSFER_DST) {
        flags |= vk::IMAGE_USAGE_TRANSFER_DST_BIT;
    }
    if usage.contains(image::COLOR_ATTACHMENT) {
        flags |= vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    }
    if usage.contains(image::DEPTH_STENCIL_ATTACHMENT) {
        flags |= vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT;
    }
    if usage.contains(image::SAMPLED) {
        flags |= vk::IMAGE_USAGE_SAMPLED_BIT;
    }

    flags
}

pub fn map_index_type(index_type: IndexType) -> vk::IndexType {
    match index_type {
        IndexType::U16 => vk::IndexType::Uint16,
        IndexType::U32 => vk::IndexType::Uint32,
    }
}

pub fn map_descriptor_type(ty: DescriptorType) -> vk::DescriptorType {
    match ty {
        DescriptorType::Sampler => vk::DescriptorType::Sampler,
        DescriptorType::SampledImage => vk::DescriptorType::SampledImage,
        DescriptorType::StorageImage => vk::DescriptorType::StorageImage,
        DescriptorType::UniformTexelBuffer => vk::DescriptorType::UniformTexelBuffer,
        DescriptorType::StorageTexelBuffer => vk::DescriptorType::StorageTexelBuffer,
        DescriptorType::ConstantBuffer => vk::DescriptorType::UniformBuffer,
        DescriptorType::StorageBuffer => vk::DescriptorType::StorageBuffer,
        DescriptorType::InputAttachment => vk::DescriptorType::InputAttachment,
    }
}

pub fn map_stage_flags(stages: shade::StageFlags) -> vk::ShaderStageFlags {
    let mut flags = vk::ShaderStageFlags::empty();

    if stages.contains(shade::STAGE_VERTEX) {
        flags |= vk::SHADER_STAGE_VERTEX_BIT;
    }

    if stages.contains(shade::STAGE_HULL) {
        flags |= vk::SHADER_STAGE_TESSELLATION_CONTROL_BIT;
    }

    if stages.contains(shade::STAGE_DOMAIN) {
        flags |= vk::SHADER_STAGE_TESSELLATION_EVALUATION_BIT;
    }

    if stages.contains(shade::STAGE_GEOMETRY) {
        flags |= vk::SHADER_STAGE_GEOMETRY_BIT;
    }

    if stages.contains(shade::STAGE_PIXEL) {
        flags |= vk::SHADER_STAGE_FRAGMENT_BIT;
    }

    if stages.contains(shade::STAGE_COMPUTE) {
        flags |= vk::SHADER_STAGE_COMPUTE_BIT;
    }

    flags
}

pub fn map_filter(filter: FilterMethod) -> (vk::Filter, vk::Filter, vk::SamplerMipmapMode, f32) {
    match filter {
        FilterMethod::Scale          => (vk::Filter::Nearest, vk::Filter::Nearest, vk::SamplerMipmapMode::Nearest, 0.0),
        FilterMethod::Mipmap         => (vk::Filter::Nearest, vk::Filter::Nearest, vk::SamplerMipmapMode::Linear,  0.0),
        FilterMethod::Bilinear       => (vk::Filter::Linear,  vk::Filter::Linear,  vk::SamplerMipmapMode::Nearest, 0.0),
        FilterMethod::Trilinear      => (vk::Filter::Linear,  vk::Filter::Linear,  vk::SamplerMipmapMode::Linear,  0.0),
        FilterMethod::Anisotropic(a) => (vk::Filter::Linear,  vk::Filter::Linear,  vk::SamplerMipmapMode::Linear,  a as f32),
    }
}

pub fn map_wrap(wrap: WrapMode) -> vk::SamplerAddressMode {
    match wrap {
        WrapMode::Tile   => vk::SamplerAddressMode::Repeat,
        WrapMode::Mirror => vk::SamplerAddressMode::MirroredRepeat,
        WrapMode::Clamp  => vk::SamplerAddressMode::ClampToEdge,
        WrapMode::Border => vk::SamplerAddressMode::ClampToBorder,
    }
}

pub fn map_border_color(col: PackedColor) -> Option<vk::BorderColor> {
    match col.0 {
        0x00000000 => Some(vk::BorderColor::FloatTransparentBlack),
        0xFF000000 => Some(vk::BorderColor::FloatOpaqueBlack),
        0xFFFFFFFF => Some(vk::BorderColor::FloatOpaqueWhite),
        _ => None
    }
}
