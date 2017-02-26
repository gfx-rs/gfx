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

//! Shader handling.

/// Shader pipeline stage
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Pixel,
    Compute,
}

/// An error type for creating shaders.
#[derive(Clone, PartialEq, Debug)]
pub enum CreateShaderError {
    /// The device does not support the requested shader model.
    ModelNotSupported,
    /// The device does not support the shader stage.
    StageNotSupported(Stage),
    /// The shader failed to compile.
    CompilationFailed(String),
    /// Library source type is not supported.
    LibrarySourceNotSupported,
}
