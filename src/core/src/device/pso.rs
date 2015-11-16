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

/// Compile-time maximum number of vertex attributes.
pub const MAX_VERTEX_ATTRIBUTES:  usize = 16;
/// An offset inside a vertex buffer, in bytes.
pub type BufferOffset = usize;

/// Error types happening upon PSO creation.
pub enum CreationError {
    /// Shader program failed to link, providing an error string.
    ProgramLink(String),
    /// Vertex attribute mismatch between the shader and the link data.
    VertexImport(d::AttributeSlot, String, Option<d::attrib::Format>),
    /// Pixel target mismatch between the shader and the link data.
    PixelExport(d::ColorSlot, String, Option<d::tex::Format>),
}

/// Compound type of the linked PSO data formats.
pub enum Link {
    /// Vertex attribute
    Attribute(d::attrib::Format),
    /// Render target
    Target(d::tex::Format),
}

/// Map of all objects that are provided for PSO usage,
/// including vertex attributes, render targets, and shader parameters.
pub type LinkMap<'a> = HashMap<&'a str, Link>;

/// Map of the resources that are actually used by the shader.
/// The values are untyped register indices.
pub type LinkResponse<'a> = HashMap<&'a str, u32>;

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

#[derive(Copy, Clone, Debug)]
/// A complete set of render targets to be used for pixel export in PSO.
pub struct PixelTargetSet<R: d::Resources>(
    /// Array of color target views
    pub [Option<R::Surface>; d::MAX_COLOR_TARGETS], //TODO
    /// Depth-stencil target view
    pub Option<R::Surface>, //TODO
);

impl<R: d::Resources> PixelTargetSet<R> {
    /// Create an empty set
    pub fn new() -> PixelTargetSet<R> {
        PixelTargetSet([None; d::MAX_COLOR_TARGETS], None)
    }
}
