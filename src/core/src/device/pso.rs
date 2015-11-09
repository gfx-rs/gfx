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

use super::{MAX_COLOR_TARGETS, attrib, tex};
use super::{AttributeSlot, ColorSlot, PrimitiveType};

/// Compile-time maximum number of vertex attributes.
pub const MAX_VERTEX_ATTRIBUTES:  usize = 16;

/// Layout of the input vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexImportLayout {
    /// Expected attribute format for every slot.
    pub formats: [Option<attrib::Format>; MAX_VERTEX_ATTRIBUTES],
}

/// Layout of the output pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct PixelExportLayout {
    /// Expected target format for every slot.
    pub colors: [Option<tex::Format>; MAX_COLOR_TARGETS],
    /// Format of the depth/stencil surface.
    pub depth_stencil: Option<tex::Format>,
}

/// Pipeline State information block.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct PipelineInfo(
    pub PrimitiveType,
    pub VertexImportLayout,
    pub PixelExportLayout,
);

impl PipelineInfo {
    /// Return the bitmask of the required render target slots
    pub fn get_export_mask(&self) -> usize {
        self.2.colors.iter().fold((0,0), |(mask, i), color| {
            (if color.is_some() { mask | (1<<i) } else { mask } , i + 1)
        }).0
    }
}

/// Error types happening upon PSO creation.
pub enum CreationError {
    /// Shader program failed to link, providing an error string.
    ProgramLink(String),
    /// Vertex attribute mismatch between the layout and the shader inputs.
    VertexImport(AttributeSlot, String),
    /// Pixel target mismatch between the layout and the shader outputs.
    PixelExport(ColorSlot, String),
}
