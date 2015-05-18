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

//! Render stream extension. Stream is something you can send batches to.
//! It includes a renderer and an output, stored by constrained types.


use device::{Device, InstanceCount, Resources, VertexCount};
use device::draw::CommandBuffer;
use device::target::{ClearData, Mask, Mirror, Rect};
use render::{DrawError, Renderer, RenderFactory};
use render::batch::Batch;
use render::target::Output;


/// Generic output window.
pub trait Window<R: Resources>: Output<R> {
    /// Swap front and back buffers.
    fn swap_buffers(&mut self);
}

/// Render stream abstraction.
pub trait Stream<R: Resources> {
    /// Command buffer type to constraint the `Renderer`.
    type CommandBuffer: CommandBuffer<R>;
    /// Constrained `Output` type.
    type Output: Output<R>;

    /// Get the output only.
    fn get_output(&self) -> &Self::Output;

    /// Access both of the stream components.
    fn access(&mut self) -> (&mut Renderer<R, Self::CommandBuffer>, &Self::Output);

    /// Get width/height aspect, needed for projections.
    fn get_aspect_ratio(&self) -> f32 {
        let (w, h) = self.get_output().get_size();
        w as f32 / h as f32
    }

    /// Clear the canvas.
    fn clear(&mut self, data: ClearData) {
        let (ren, out) = self.access();
        let mask = out.get_mask();
        ren.clear(data, mask, out);
    }

    /// Blit on this stream from another `Output`.
    fn blit_on<I: Output<R>>(&mut self,
               source: &I, source_rect: Rect, dest_rect: Rect,
               mirror: Mirror, mask: Mask) {
        let (ren, out) = self.access();
        ren.blit(source, source_rect, out, dest_rect, mirror, mask);
    }

    /// Blit this stream to another `Output`.
    fn blit_to<O: Output<R>>(&mut self,
               destination: &O, dest_rect: Rect, source_rect: Rect,
               mirror: Mirror, mask: Mask) {
        let (ren, out) = self.access();
        ren.blit(out, source_rect, destination, dest_rect, mirror, mask);
    }

    /// Draw a simple `Batch`.
    fn draw<B: Batch<R>>(&mut self, batch: &B) 
            -> Result<(), DrawError<B::Error>> {
        let (ren, out) = self.access();
        ren.draw(batch, out)
    }

    /// Draw an instanced `Batch`.
    fn draw_instanced<B: Batch<R>>(&mut self, batch: &B,
                      count: InstanceCount, base: VertexCount)
                      -> Result<(), DrawError<B::Error>> {
        let (ren, out) = self.access();
        ren.draw_instanced(batch, count, base, out)
    }

    /// Execute everything and clear the command buffer.
    fn flush<D>(&mut self, device: &mut D) where
        D: Device<Resources = R, CommandBuffer = Self::CommandBuffer>,
    {
        let (ren, _) = self.access();
        device.submit(ren.as_buffer());
        ren.reset();
    }
}

impl<'a, R: Resources, C: CommandBuffer<R>, O: Output<R>>
Stream<R> for (&'a mut Renderer<R, C>, &'a O) {
    type CommandBuffer = C;
    type Output = O;

    fn get_output(&self) -> &O {
        &self.1
    }

    fn access(&mut self) -> (&mut Renderer<R, C>, &O) {
        (&mut self.0, &self.1)
    }
}

/// A stream that owns its components.
pub struct OwnedStream<
    R: Resources,
    C: CommandBuffer<R>,
    O: Output<R>,
>{
    /// Renderer
    pub ren: Renderer<R, C>,
    /// Output
    pub out: O,
}

impl<R: Resources, C: CommandBuffer<R>, O: Output<R>>
Stream<R> for OwnedStream<R, C, O> {
    type CommandBuffer = C;
    type Output = O;

    fn get_output(&self) -> &O {
        &self.out
    }

    fn access(&mut self) -> (&mut Renderer<R, C>, &O) {
        (&mut self.ren, &self.out)
    }
}

impl<D: Device, W: Window<D::Resources>> OwnedStream<D::Resources, D::CommandBuffer, W> {
    /// Show what we've been drawing all this time.
    pub fn present(&mut self, device: &mut D) {
        self.flush(device);
        self.out.swap_buffers();
        device.cleanup();
    }
}

/// A render factory extension that allows creating streams with new renderers.
pub trait StreamFactory<R: Resources, C: CommandBuffer<R>>: RenderFactory<R, C> {
    /// Create a new stream from a given output.
    fn create_stream<O: Output<R>>(&mut self, output: O) -> OwnedStream<R, C, O> {
        OwnedStream {
            ren: self.create_renderer(),
            out: output,
        }
    }
}

impl<R: Resources, C: CommandBuffer<R>, F: RenderFactory<R, C>>
StreamFactory<R, C> for F {}
