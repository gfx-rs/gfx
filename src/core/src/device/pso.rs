// Copyright 2015 The Gfx-rs Developers.
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

//! Pipeline State Objects

use std::collections::HashMap;
use device as d;
use state as s;

/// Compile-time maximum number of vertex attributes.
pub const MAX_VERTEX_ATTRIBUTES: usize = 16;
/// Compile-time maximum number of constant buffers.
pub const MAX_CONSTANT_BUFFERS: usize = 16;
/// An offset inside a vertex buffer, in bytes.
pub type BufferOffset = usize;
/// A special unique tag for depth/stencil entries in the Link/Register maps.
pub const DEPTH_STENCIL_TAG: &'static str = "<ds>";

/// Error types happening upon PSO creation.
pub enum CreationError {
    /// Shader program failed to link, providing an error string.
    ProgramLink(String),
    /// Vertex attribute mismatch between the shader and the link data.
    VertexImport(d::AttributeSlot, String, Option<d::attrib::Format>),
    /// Pixel target mismatch between the shader and the link data.
    PixelExport(d::ColorSlot, String, Option<d::tex::Format>),
}

#[allow(missing_docs)]
#[derive(Clone, Copy)]
pub struct BlendInfo {
    pub mask: s::ColorMask,
    pub color: Option<s::BlendChannel>,
    pub alpha: Option<s::BlendChannel>,
}
impl From<s::ColorMask> for BlendInfo {
    fn from(mask: s::ColorMask) -> BlendInfo {
        BlendInfo {
            mask: mask,
            color: None,
            alpha: None,
        }
    }
}
impl From<s::Blend> for BlendInfo {
    fn from(blend: s::Blend) -> BlendInfo {
        BlendInfo {
            mask: blend.mask,
            color: Some(blend.color),
            alpha: Some(blend.alpha),
        }
    }
}

#[allow(missing_docs)]
#[derive(Clone, Copy)]
pub struct DepthStencilInfo {
    pub depth: Option<s::Depth>,
    pub front: Option<s::StencilSide>,
    pub back: Option<s::StencilSide>,
}
impl From<s::Depth> for DepthStencilInfo {
    fn from(depth: s::Depth) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(depth),
            front: None,
            back: None,
        }
    }
}
impl From<s::Stencil> for DepthStencilInfo {
    fn from(stencil: s::Stencil) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: None,
            front: Some(stencil.front),
            back: Some(stencil.back),
        }
    }
}
impl From<(s::Depth, s::Stencil)> for DepthStencilInfo {
    fn from(ds: (s::Depth, s::Stencil)) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(ds.0),
            front: Some(ds.1.front),
            back: Some(ds.1.back),
        }
    }
}

/// Compound type of the linked PSO data formats.
pub enum Link {
    /// Vertex attribute
    Attribute(d::attrib::Format),
    /// Uniform constant, may be inside a constant buffer, or outside
    Constant(d::attrib::Format),
    /// Constant buffer
    ConstantBuffer,
    /// Shader resource view (SRV)
    Resource,
    /// Unordered access view (UAV)
    Unordered,
    /// Render target view (RTV)
    Target(d::tex::Format, BlendInfo),
    /// Depth stencil view (DSV)
    DepthStencil(d::tex::Format, DepthStencilInfo),
}

/// Map of all objects that are provided for PSO usage,
/// including vertex attributes, render targets, and shader parameters.
pub type LinkMap<'a> = HashMap<&'a str, Link>;

/// Generic (untyped) register.
pub type Register = u32;

/// Map of the registers that are actually used by the shader.
pub type RegisterMap<'a> = HashMap<&'a str, Register>;

/// Layout of the input vertices.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct VertexImportLayout {
    /// Expected attribute format for every slot.
    pub formats: [Option<d::attrib::Format>; MAX_VERTEX_ATTRIBUTES],
}

fn match_attribute(_sh: &d::shade::Attribute, _format: d::attrib::Format) -> bool {
    true //TODO
}

impl VertexImportLayout {
    /// Create an empty layout
    pub fn new() -> VertexImportLayout {
        VertexImportLayout {
            formats: [None; MAX_VERTEX_ATTRIBUTES],
        }
    }
    /// Create the layout by matching shader requirements with the link map.
    pub fn link(map: &LinkMap, attributes: &[d::shade::Attribute])
                -> Result<VertexImportLayout, CreationError> {
        let mut formats = [None; MAX_VERTEX_ATTRIBUTES];
        for at in attributes.iter() {
            let slot = at.location as d::AttributeSlot;
            match map.get(&at.name[..]) {
                Some(&Link::Attribute(fm)) => {
                    if match_attribute(at, fm) {
                        formats[at.location] = Some(fm);
                    }else {
                        return Err(CreationError::VertexImport(slot, at.name.clone(), Some(fm)))
                    }
                },
                _ => return Err(CreationError::VertexImport(slot, at.name.clone(), None))
            }
        }
        Ok(VertexImportLayout {
            formats: formats,
        })
    }
}

/// Layout of the output pixels.
#[derive(Clone, Copy, Debug, Hash, PartialEq)]
pub struct PixelExportLayout {
    /// Expected target format for every slot.
    pub colors: [Option<d::tex::Format>; d::MAX_COLOR_TARGETS],
    /// Format of the depth/stencil surface.
    pub depth_stencil: Option<d::tex::Format>,
}

impl PixelExportLayout {
    /// Create an empty layout
    pub fn new() -> PixelExportLayout {
        PixelExportLayout {
            colors: [None; d::MAX_COLOR_TARGETS],
            depth_stencil: None,
        }
    }
    /// Create the layout by matching shader requirements with the link map.
    pub fn link(_map: &LinkMap, _outputs: &[d::shade::Output], need_depth: bool)
                -> Result<PixelExportLayout, CreationError> {
        let mut colors = [None; d::MAX_COLOR_TARGETS];
        let depth_stencil = if need_depth {
            Some(d::tex::Format::DEPTH24_STENCIL8)
        } else {None};
        colors[0] = Some(d::tex::RGBA8); //TODO
        Ok(PixelExportLayout {
            colors: colors,
            depth_stencil: depth_stencil,
        })
    }
    /// Return the bitmask of the required render target slots
    pub fn get_mask(&self) -> usize {
        self.colors.iter().fold((0,0), |(mask, i), color| {
            (if color.is_some() { mask | (1<<i) } else { mask } , i + 1)
        }).0
    }
}

/// A complete set of vertex buffers to be used for vertex import in PSO.
#[derive(Copy, Clone, Debug)]
pub struct VertexBufferSet<R: d::Resources>(
    /// Array of buffer handles with offsets in them
    pub [Option<(R::Buffer, BufferOffset)>; MAX_VERTEX_ATTRIBUTES]
);

impl<R: d::Resources> VertexBufferSet<R> {
    /// Create an empty set
    pub fn new() -> VertexBufferSet<R> {
        VertexBufferSet([None; MAX_VERTEX_ATTRIBUTES])
    }
}

/// A complete set of constant buffers to be used for the constants binding in PSO.
#[derive(Copy, Clone, Debug)]
pub struct ConstantBufferSet<R: d::Resources>(
    /// Array of buffer handles
    pub [Option<R::Buffer>; MAX_CONSTANT_BUFFERS]
);

impl<R: d::Resources> ConstantBufferSet<R> {
    /// Create an empty set
    pub fn new() -> ConstantBufferSet<R> {
        ConstantBufferSet([None; MAX_CONSTANT_BUFFERS])
    }
}

#[derive(Copy, Clone, Debug)]
/// A complete set of render targets to be used for pixel export in PSO.
pub struct PixelTargetSet<R: d::Resources>(
    /// Array of color target views
    pub [Option<R::RenderTargetView>; d::MAX_COLOR_TARGETS],
    /// Depth-stencil target view
    pub Option<R::DepthStencilView>,
);

impl<R: d::Resources> PixelTargetSet<R> {
    /// Create an empty set
    pub fn new() -> PixelTargetSet<R> {
        PixelTargetSet([None; d::MAX_COLOR_TARGETS], None)
    }
}

/// The rasterizer state needed for PSO initialization.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Rasterizer {
    /// Type of the primitive.
    pub topology: d::PrimitiveType,
    /// Which vertex winding is considered to be the front face for culling.
    pub front_face: s::FrontFace,
    /// How to rasterize this primitive.
    pub raster_method: s::RasterMethod,
    /// Depth offset to apply.
    pub depth_offset: Option<s::Offset>,
    /// Multi-sampling mode.
    pub multi_sample: Option<s::MultiSample>,
}
