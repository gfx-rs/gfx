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

use {pso, shade};
use {Resources, SubPass};

/// A `Factory` is responsible for creating and managing resources for the backend it was created
/// with.
///
/// This factory structure can then be used to create and manage different resources, like buffers,
/// pipelines and textures. See the individual methods for more information.
#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// 
    // fn allocate_memory(&mut self);

    ///
    fn create_renderpass(&mut self) -> R::RenderPass;

    ///
    fn create_pipeline_signature(&mut self) -> R::PipelineSignature;

    ///
    fn create_graphics_pipelines<'a>(&mut self, &[(&R::ShaderLib, &R::PipelineSignature, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<R::PipelineStateObject, pso::CreationError>>;

    ///
    fn create_compute_pipelines(&mut self) -> Vec<Result<R::PipelineStateObject, pso::CreationError>>;
}
