// Copyright 2014 The Gfx-rs Developers.
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

//! Graphics commands encoder.

#![deny(missing_docs)]

use std::mem;
use draw_state::target::{Depth, Stencil};

use gfx_core::{Device, Factory, IndexType, Resources, SubmitInfo, VertexCount};
use gfx_core::{draw, format, handle, tex};
use gfx_core::factory::Phantom;
use mesh;
use pso;


/// An error occuring in buffer/texture updates.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateError<T> {
    OutOfBounds {
        target: T,
        source: T,
    },
    UnitCountMismatch {
        target: usize,
        slice: usize,
    },
}


/// Graphics commands encoder.
pub struct Encoder<R: Resources, C: draw::CommandBuffer<R>> {
    command_buffer: C,
    data_buffer: draw::DataBuffer,
    handles: handle::Manager<R>,
}

impl<R: Resources, C: draw::CommandBuffer<R>> Encoder<R, C> {
    /// Create a new encoder using a factory.
    pub fn new(cb: C) -> Encoder<R, C> {
        Encoder {
            command_buffer: cb,
            data_buffer: draw::DataBuffer::new(),
            handles: handle::Manager::new(),
        }
    }

    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.command_buffer.reset();
        self.data_buffer.clear();
        self.handles.clear();
    }

    /// Get command and data buffers to be submitted to the device.
    pub fn as_buffer<D>(&self) -> SubmitInfo<D> where
        D: Device<Resources=R, CommandBuffer=C> {
        SubmitInfo(&self.command_buffer, &self.data_buffer, &self.handles)
    }

    /// Clone the renderer shared data but ignore the commands.
    pub fn clone_empty(&self) -> Encoder<R, C> {
        Encoder {
            command_buffer: self.command_buffer.clone_empty(),
            data_buffer: draw::DataBuffer::new(),
            handles: handle::Manager::new(),
        }
    }

    /// Update a buffer with a slice of data.
    pub fn update_buffer<T: Copy>(&mut self, buf: &handle::Buffer<R, T>,
                         data: &[T], offset_elements: usize)
                         -> Result<(), UpdateError<usize>>
    {
        if data.is_empty() {
            return Ok(())
        }
        let elem_size = mem::size_of::<T>();
        let offset_bytes = elem_size * offset_elements;
        let bound = data.len().wrapping_mul(elem_size) + offset_bytes;
        if bound <= buf.get_info().size {
            let pointer = self.data_buffer.add_vec(data);
            self.command_buffer.update_buffer(
                self.handles.ref_buffer(buf.raw()).clone(),
                pointer, offset_bytes);
            Ok(())
        } else {
            Err(UpdateError::OutOfBounds {
                target: bound,
                source: buf.get_info().size,
            })
        }
    }

    /// Update a buffer with a single structure.
    pub fn update_constant_buffer<T: Copy>(&mut self, buf: &handle::Buffer<R, T>, data: &T) {
        let pointer = self.data_buffer.add_struct(data);
        self.command_buffer.update_buffer(
            self.handles.ref_buffer(buf.raw()).clone(),
            pointer, 0);
    }

    /// Update the contents of a texture.
    pub fn update_texture<S, T>(&mut self, tex: &handle::Texture<R, T>,
                          face: Option<tex::CubeFace>,
                          img: tex::NewImageInfo, data: &[S::DataType])
                          -> Result<(), UpdateError<[tex::Size; 3]>>
    where
        S: format::SurfaceTyped,
        S::DataType: Copy,
        T: format::Formatted<Surface = S>,
    {
        if data.is_empty() {
            return Ok(())
        }

        let target_count = img.get_texel_count();
        if target_count != data.len() {
            return Err(UpdateError::UnitCountMismatch {
                target: target_count,
                slice: data.len(),
            })
        }

        let dim = tex.get_info().kind.get_dimensions();
        if !img.is_inside(dim) {
            let (w, h, d, _) = dim;
            return Err(UpdateError::OutOfBounds {
                target: [
                    img.xoffset + img.width,
                    img.yoffset + img.height,
                    img.zoffset + img.depth,
                ],
                source: [w, h, d],
            })
        }

        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_texture(
            self.handles.ref_texture(tex.raw()).clone(),
            tex.get_info().kind, face, pointer,
            img.convert(T::get_format()));
        Ok(())
    }

    fn draw_indexed<T>(&mut self, buf: &handle::Buffer<R, T>, ty: IndexType,
                     slice: &mesh::Slice<R>, base: VertexCount,
                     instances: draw::InstanceOption) {
        self.command_buffer.bind_index(self.handles.ref_buffer(buf.raw()).clone(), ty);
        self.command_buffer.call_draw_indexed(slice.start, slice.end - slice.start, base, instances);
    }

    fn draw_slice(&mut self, slice: &mesh::Slice<R>, instances: draw::InstanceOption) {
        match slice.kind {
            mesh::SliceKind::Vertex => self.command_buffer.call_draw(
                slice.start, slice.end - slice.start, instances),
            mesh::SliceKind::Index8(ref buf, base) =>
                self.draw_indexed(buf, IndexType::U8, slice, base, instances),
            mesh::SliceKind::Index16(ref buf, base) =>
                self.draw_indexed(buf, IndexType::U16, slice, base, instances),
            mesh::SliceKind::Index32(ref buf, base) =>
                self.draw_indexed(buf, IndexType::U32, slice, base, instances),
        }
    }

    /// Clear a target view with a specified value.
    pub fn clear<T: format::RenderFormat>(&mut self,
                 view: &handle::RenderTargetView<R, T>, value: T::View)
    where T::View: Into<draw::ClearColor> {
        let target = self.handles.ref_rtv(view.raw()).clone();
        self.command_buffer.clear_color(target, value.into())
    }
    /// Clear a depth view with a specified value.
    pub fn clear_depth<T: format::DepthFormat>(&mut self,
                       view: &handle::DepthStencilView<R, T>, depth: Depth) {
        let target = self.handles.ref_dsv(view.raw()).clone();
        self.command_buffer.clear_depth_stencil(target, Some(depth), None)
    }

    /// Clear a stencil view with a specified value.
    pub fn clear_stencil<T: format::StencilFormat>(&mut self,
                         view: &handle::DepthStencilView<R, T>, stencil: Stencil) {
        let target = self.handles.ref_dsv(view.raw()).clone();
        self.command_buffer.clear_depth_stencil(target, None, Some(stencil))
    }

    /// Draw a mesh slice using a typed pipeline state object (PSO).
    pub fn draw<D: pso::PipelineData<R>>(&mut self, slice: &mesh::Slice<R>,
                pipeline: &pso::PipelineState<R, D::Meta>, user_data: &D)
    {
        let (pso, _) = self.handles.ref_pso(pipeline.get_handle());
        self.command_buffer.bind_pipeline_state(pso.clone());
        let raw_data = user_data.bake(pipeline.get_meta(), &mut self.handles);
        self.command_buffer.bind_vertex_buffers(raw_data.vertex_buffers);
        self.command_buffer.bind_constant_buffers(raw_data.constant_buffers);
        for &(location, value) in &raw_data.global_constants {
            self.command_buffer.bind_global_constant(location, value);
        }
        self.command_buffer.bind_resource_views(raw_data.resource_views);
        self.command_buffer.bind_unordered_views(raw_data.unordered_views);
        self.command_buffer.bind_samplers(raw_data.samplers);
        self.command_buffer.bind_pixel_targets(raw_data.pixel_targets);
        self.command_buffer.set_ref_values(raw_data.ref_values);
        self.command_buffer.set_scissor(raw_data.scissor);
        self.draw_slice(slice, slice.instances);
    }
}
