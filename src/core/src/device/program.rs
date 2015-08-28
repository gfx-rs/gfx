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


//! Program creating and modification


use super::Resources;
use handle;

/// A program builder is used to `bind` shader/target/transform_vearyings
/// together
pub struct Builder<'a, R>
    where R: Resources+'a,
          R::Buffer: 'a,
          R::ArrayBuffer: 'a,
          R::Shader: 'a,
          R::Program: 'a,
          R::FrameBuffer: 'a,
          R::Surface: 'a,
          R::Texture: 'a,
          R::Sampler: 'a,
          R::Fence: 'a

{
    /// The shaders bound to the program
    pub shaders: Vec<&'a handle::Shader<R>>,
    /// the targets for the output
    pub targets: Vec<&'a str>,

    /* TODO
    /// the transform_varyings for transform feedback
    pub transform_varyings: Vec<&'a str>
    */
}

impl<'a, R> Builder<'a, R>
    where R: Resources+'a,
          R::Buffer: 'a,
          R::ArrayBuffer: 'a,
          R::Shader: 'a,
          R::Program: 'a,
          R::FrameBuffer: 'a,
          R::Surface: 'a,
          R::Texture: 'a,
          R::Sampler: 'a,
          R::Fence: 'a
{
    /// Create's a new program builder
    pub fn new() -> Builder<'a, R> {
        Builder{
            shaders: Vec::new(),
            targets: Vec::new(),
            //transform_varyings: Vec::new()
        }
    }

    /// add a shader into to the program
    pub fn add_shader(mut self, shader: &'a handle::Shader<R>) -> Builder<'a, R> {
        self.shaders.push(shader);
        self
    }

    /// add multiple shaders
    pub fn add_shaders(mut self, shaders: &[&'a handle::Shader<R>]) -> Builder<'a, R> {
        for s in shaders {
            self = self.add_shader(s);
        }
        self
    }

    /// add a target to the shader
    pub fn add_target(mut self, target: &'a str) -> Builder<'a, R> {
        self.targets.push(target);
        self
    }

    /// add multiple targets
    pub fn add_targets(mut self, targets: &[&'a str]) -> Builder<'a, R> {
        for t in targets {
            self = self.add_target(t);
        }
        self
    }

    /* TODO
    /// add a transform varying binding point
    pub fn add_transform_varying(mut self, varying: &'a str) -> Builder<'a, R> {
        self.transform_varyings.push(varying);
        self
    }
    */
}