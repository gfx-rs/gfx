// Copyright 2016 The Gfx-rs Developers.
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

use gfx_core::{factory as f, tex};
use vk;


pub fn map_image_type(kind: tex::Kind) -> vk::ImageType {
   match kind {
        tex::Kind::D1(..) | tex::Kind::D1Array(..) => vk::IMAGE_TYPE_1D,
        tex::Kind::D2(..) | tex::Kind::D2Array(..) => vk::IMAGE_TYPE_2D,
        tex::Kind::D3(..) => vk::IMAGE_TYPE_3D,
        tex::Kind::Cube(..) | tex::Kind::CubeArray(..) => vk::IMAGE_TYPE_2D,
    }
}

pub fn map_usage_tiling(gfx_usage: f::Usage, bind: f::Bind) -> (vk::ImageUsageFlags, vk::ImageTiling) {
    let mut usage = 0;
    if bind.contains(f::RENDER_TARGET) {
        usage |= vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    }
    if bind.contains(f::DEPTH_STENCIL) {
        usage |= vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT;
    }
    if bind.contains(f::SHADER_RESOURCE) {
        usage |= vk::IMAGE_USAGE_SAMPLED_BIT;
    }
    if bind.contains(f::UNORDERED_ACCESS) {
        usage |= vk::IMAGE_USAGE_STORAGE_BIT;
    }
    let tiling = match gfx_usage {
        f::Usage::Const => vk::IMAGE_TILING_OPTIMAL,
        f::Usage::GpuOnly => {
            //TODO: not always needed
            usage |= vk::IMAGE_USAGE_TRANSFER_SRC_BIT | vk::IMAGE_USAGE_TRANSFER_DST_BIT;
            vk::IMAGE_TILING_OPTIMAL
        },
        f::Usage::Dynamic => {
            usage |= vk::IMAGE_USAGE_TRANSFER_DST_BIT;
            vk::IMAGE_TILING_LINEAR
        },
        f::Usage::CpuOnly(map) => {
            usage |= match map {
                f::MapAccess::Readable => vk::IMAGE_USAGE_TRANSFER_DST_BIT,
                f::MapAccess::Writable => vk::IMAGE_USAGE_TRANSFER_SRC_BIT,
                f::MapAccess::RW => vk::IMAGE_USAGE_TRANSFER_SRC_BIT | vk::IMAGE_USAGE_TRANSFER_DST_BIT,
            };
            vk::IMAGE_TILING_LINEAR
        },
    };
    (usage, tiling)
}
