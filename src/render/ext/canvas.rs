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

use draw_state::target::{ClearData, Mask, Mirror, Rect};
use device::{Device, Factory, Resources};
use device::{InstanceCount, VertexCount};
use render::{DrawError, Renderer};
use render::batch::Batch;
use render::target::Output;


/// Generic output window.
pub trait Window<R: Resources>: Output<R> {
    /// Swap front and back buffers.
    fn swap_buffers(&mut self);
}

/// A canvas with everything you need to draw on it.
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

/// Something that can be transformed into `Canvas`.
pub trait IntoCanvas<W, D: Device, F> {
    /// Transform into `Canvas`.
    fn into_canvas(self) -> Canvas<W, D, F>;
}

impl<W, D: Device, F: Factory<D::Resources>> IntoCanvas<W, D, F> for (W, D, F) {
    fn into_canvas(mut self) -> Canvas<W, D, F> {
        use super::factory::RenderFactory;
        let renderer = self.2.create_renderer();
        Canvas {
            output: self.0,
            device: self.1,
            factory: self.2,
            renderer: renderer,
        }
    }
}

impl<D: Device, F: Factory<D::Resources>, W: Window<D::Resources>> Canvas<W, D, F> {
    /// Get width/height aspect, needed for projections.
    pub fn get_aspect_ratio(&self) -> f32 {
        let (w, h) = self.output.get_size();
        w as f32 / h as f32
    }

    /// Clear the canvas.
    pub fn clear(&mut self, data: ClearData) {
        let mask = self.output.get_mask();
        self.renderer.clear(data, mask, &self.output);
    }

    /// Blit on this canvas from another `Output`.
    pub fn blit_on<I: Output<D::Resources>>(&mut self,
                   source: &I, source_rect: Rect, dest_rect: Rect,
                   mirror: Mirror, mask: Mask) {
        self.renderer.blit(source, source_rect, &self.output, dest_rect, mirror, mask);
    }

    /// Blit this canvas to another `Output`.
    pub fn blit_to<O: Output<D::Resources>>(&mut self,
                   destination: &O, dest_rect: Rect, source_rect: Rect,
                   mirror: Mirror, mask: Mask) {
        self.renderer.blit(&self.output, source_rect, destination, dest_rect, mirror, mask);
    }

    /// Draw a simple `Batch`.
    pub fn draw<B: Batch<Resources = D::Resources>>(&mut self, batch: &B)
                -> Result<(), DrawError<B::Error>> {
        self.renderer.draw_all(batch, None, &self.output)
    }

    /// Draw an instanced `Batch`.
    pub fn draw_instanced<B: Batch<Resources = D::Resources>>(&mut self, batch: &B,
                          count: InstanceCount, base: VertexCount)
                          -> Result<(), DrawError<B::Error>> {
        self.renderer.draw_all(batch, Some((count, base)), &self.output)
    }

    /// Show what we've been drawing all this time.
    pub fn present(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.output.swap_buffers();
        self.device.after_frame();
        self.factory.cleanup();
        self.renderer.reset();
    }
}
