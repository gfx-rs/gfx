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

use device::{Device, Factory, Resources};
use extra::stream::Stream;
use render::{Renderer, RenderFactory};
use render::target::Output;


/// Generic output window.
pub trait Window<R: Resources>: Output<R> {
    /// Swap front and back buffers.
    fn swap_buffers(&mut self);
}

/// DEPRECATED
pub struct Canvas<W, D: Device, F> {
    /// Output window.
    pub output: W,
    /// Graphics device.
    pub device: D,
    /// Resource factory.
    pub factory: F,
    /// Renderer front-end.
    pub renderer: Renderer<D::Resources, D::CommandBuffer>,
}

impl<W, D: Device, F: Factory<D::Resources>> From<(W, D, F)> for Canvas<W, D, F> {
    fn from(mut triple: (W, D, F)) -> Canvas<W, D, F> {
        let renderer = triple.2.create_renderer();
        Canvas {
            output: triple.0,
            device: triple.1,
            factory: triple.2,
            renderer: renderer,
        }
    }
}

impl<D: Device, F: Factory<D::Resources>, O: Output<D::Resources>>
Stream<D::Resources> for Canvas<O, D, F> {
    type CommandBuffer = D::CommandBuffer;
    type Output = O;

    fn get_output(&self) -> &O {
        &self.output
    }

    fn access(&mut self) -> (&mut Renderer<D::Resources, D::CommandBuffer>, &O) {
        (&mut self.renderer, &self.output)
    }
}

impl<D: Device, F: Factory<D::Resources>, W: Window<D::Resources>> Canvas<W, D, F> {
    /// Show what we've been drawing all this time.
    pub fn present(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.output.swap_buffers();
        self.device.cleanup();
        self.renderer.reset();
    }
}
