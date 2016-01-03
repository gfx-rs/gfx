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
use draw_state::target::{ClearData, ColorValue, Depth, Mask, Stencil};

use gfx_core as device;
use gfx_core::Resources;
use gfx_core::{format, handle};
use gfx_core::attrib::IntSize;
use gfx_core::draw::{CommandBuffer, DataBuffer, InstanceOption};
use gfx_core::factory::{Factory, NotSupported};
use gfx_core::output::Output;
use gfx_core::tex::Size;
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
    UnitSizeMismatch {
        target: u8,
        source: u8,
    },
    UnitCountMismatch {
        target: usize,
        slice: usize,
    },
}


/// Graphics commands encoder.
pub struct Encoder<R: Resources, C: CommandBuffer<R>> {
    command_buffer: C,
    data_buffer: DataBuffer,
    handles: handle::Manager<R>,
    common_array_buffer: Result<handle::ArrayBuffer<R>, NotSupported>,
    draw_frame_buffer: Result<handle::FrameBuffer<R>, NotSupported>,
    read_frame_buffer: Result<handle::FrameBuffer<R>, NotSupported>,
}

impl<R: Resources, C: CommandBuffer<R>> Encoder<R, C> {
    /// Create a new encoder using a factory.
    pub fn create<F>(factory: &mut F) -> Encoder<R, C> where
        F: Factory<R, CommandBuffer = C>
    {
        Encoder {
            command_buffer: factory.create_command_buffer(),
            data_buffer: DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: factory.create_array_buffer(),
            draw_frame_buffer: factory.create_frame_buffer(),
            read_frame_buffer: factory.create_frame_buffer(),
        }
    }

    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.command_buffer.clear();
        self.data_buffer.clear();
        self.handles.clear();
    }

    /// Get command and data buffers to be submitted to the device.
    pub fn as_buffer<D>(&self) -> device::SubmitInfo<D> where
        D: device::Device<Resources=R, CommandBuffer=C> {
        device::SubmitInfo(&self.command_buffer, &self.data_buffer, &self.handles)
    }

    /// Clone the renderer shared data but ignore the commands.
    pub fn clone_empty(&self) -> Encoder<R, C> {
        Encoder {
            command_buffer: self.command_buffer.clone_empty(),
            data_buffer: DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: self.common_array_buffer.clone(),
            draw_frame_buffer: self.draw_frame_buffer.clone(),
            read_frame_buffer: self.read_frame_buffer.clone(),
        }
    }

    /// Update a buffer with a slice of data.
    pub fn update_buffer<T: Copy>(&mut self, buf: &handle::RawBuffer<R>,
                         data: &[T], offset_elements: usize)
                         -> Result<(), UpdateError<usize>>
    {
        if data.is_empty() {
            return Ok(())
        }
        let elem_size = mem::size_of::<T>();
        let offset_bytes = elem_size * offset_elements;
        let bound = data.len() * elem_size + offset_bytes;
        if bound <= buf.get_info().size {
            let pointer = self.data_buffer.add_vec(data);
            self.command_buffer.update_buffer(
                self.handles.ref_buffer(buf).clone(),
                pointer, offset_bytes);
            Ok(())
        } else {
            Err(UpdateError::OutOfBounds {
                target: bound,
                source: buf.get_info().size,
            })
        }
    }

    /// Update a buffer with a data struct.
    pub fn update_block<U, T: Copy>(&mut self, buf: &handle::Buffer<R, U>, data: &T)
                        -> Result<(), UpdateError<usize>>
    {
        let bound = mem::size_of::<T>();
        if bound <= buf.get_info().size {
            use gfx_core::factory::Phantom;
            let pointer = self.data_buffer.add_struct(data);
            self.command_buffer.update_buffer(
                self.handles.ref_buffer(buf.raw()).clone(),
                pointer, 0);
            Ok(())
        } else {
            Err(UpdateError::OutOfBounds {
                target: bound,
                source: buf.get_info().size,
            })
        }
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Copy>(&mut self, tex: &handle::Texture<R>,
                          img: device::tex::ImageInfo, data: &[T])
                          -> Result<(), UpdateError<[Size; 3]>>
    {
        if data.is_empty() {
            return Ok(())
        }

        let source_size = tex.get_info().format.get_size().unwrap_or(0);
        let target_size = mem::size_of::<T>() as u8;
        if source_size != target_size {
            return Err(UpdateError::UnitSizeMismatch {
                target: target_size,
                source: source_size,
            })
        }

        let target_count = img.get_texel_count();
        if target_count != data.len() {
            return Err(UpdateError::UnitCountMismatch {
                target: target_count,
                slice: data.len(),
            })
        }

        if !tex.get_info().contains(&img) {
            let (w, h, d, _) = tex.get_info().kind.get_dimensions();
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
        self.command_buffer.update_texture(tex.get_info().kind,
            self.handles.ref_texture(tex).clone(), img, pointer);
        Ok(())
    }

    fn draw_indexed<T>(&mut self, buf: &handle::Buffer<R, T>, format: IntSize,
                     slice: &mesh::Slice<R>, base: device::VertexCount,
                     instances: InstanceOption) {
        use gfx_core::factory::Phantom;
        self.command_buffer.bind_index(self.handles.ref_buffer(buf.raw()).clone());
        self.command_buffer.call_draw_indexed(format,
            slice.start, slice.end - slice.start, base, instances);
    }

    fn draw_slice(&mut self, slice: &mesh::Slice<R>, instances: InstanceOption) {
        match slice.kind {
            mesh::SliceKind::Vertex => self.command_buffer.call_draw(
                slice.start, slice.end - slice.start, instances),
            mesh::SliceKind::Index8(ref buf, base) =>
                self.draw_indexed(buf, IntSize::U8, slice, base, instances),
            mesh::SliceKind::Index16(ref buf, base) =>
                self.draw_indexed(buf, IntSize::U16, slice, base, instances),
            mesh::SliceKind::Index32(ref buf, base) =>
                self.draw_indexed(buf, IntSize::U32, slice, base, instances),
        }
    }

    fn clear_all<T>(&mut self,
                 color: Option<(&handle::RenderTargetView<R, T>, ColorValue)>,
                 depth: Option<(&handle::DepthStencilView<R, T>, Depth)>,
                 stencil: Option<(&handle::DepthStencilView<R, T>, Stencil)>)
    {
        use draw_state::target::{COLOR, DEPTH, STENCIL};
        use gfx_core::factory::Phantom;
        use gfx_core::pso::PixelTargetSet;

        let mut pts = PixelTargetSet::new();
        let mut mask = Mask::empty();
        let mut cdata = ClearData {
            color: [0.0; 4],
            depth: 0.0,
            stencil: 0,
        };
        if let Some((view, c)) = color {
            pts.colors[0] = Some(self.handles.ref_rtv(view.raw()).clone());
            mask = mask | COLOR;
            cdata.color = c;
        }
        if let Some((view, d)) = depth {
            pts.depth = Some(self.handles.ref_dsv(view.raw()).clone());
            mask = mask | DEPTH;
            cdata.depth = d;
        }
        if let Some((view, s)) = stencil {
            pts.stencil = Some(self.handles.ref_dsv(view.raw()).clone());
            mask = mask | STENCIL;
            cdata.stencil = s;
        }
        self.command_buffer.bind_pixel_targets(pts);
        self.command_buffer.call_clear(cdata, mask);
    }

    /// Clear a target view with a specified value.
    pub fn clear<T: format::RenderFormat>(&mut self,
                 view: &handle::RenderTargetView<R, T>, value: ColorValue) { //TODO: value: T
        self.clear_all(Some((view, value)), None, None)
    }
    /// Clear a depth view with a specified value.
    pub fn clear_depth<T: format::DepthFormat>(&mut self,
                       view: &handle::DepthStencilView<R, T>, depth: Depth) {
        self.clear_all(None, Some((view, depth)), None)
    }

    /// Clear a stencil view with a specified value.
    pub fn clear_stencil<T: format::StencilFormat>(&mut self,
                         view: &handle::DepthStencilView<R, T>, stencil: Stencil) {
        self.clear_all(None, None, Some((view, stencil)))
    }

    /// Draw a mesh slice using a typed pipeline state object (PSO).
    pub fn draw<D: pso::PipelineData<R>>(&mut self, slice: &mesh::Slice<R>,
                pipeline: &pso::PipelineState<R, D::Meta>, user_data: &D)
    {
        let (pso, _) = self.handles.ref_pso(pipeline.get_handle());
        self.command_buffer.bind_pipeline_state(pso.clone());
        let raw_data = pipeline.prepare_data(user_data, &mut self.handles);
        self.command_buffer.bind_vertex_buffers(raw_data.vertex_buffers);
        self.command_buffer.bind_constant_buffers(raw_data.constant_buffers);
        for &(location, value) in &raw_data.global_constants {
            self.command_buffer.bind_global_constant(location, value);
        }
        self.command_buffer.bind_resource_views(raw_data.resource_views);
        self.command_buffer.bind_unordered_views(raw_data.unordered_views);
        self.command_buffer.bind_samplers(raw_data.samplers);
        self.command_buffer.bind_pixel_targets(raw_data.pixel_targets);
        self.draw_slice(slice, slice.instances);
    }
}
