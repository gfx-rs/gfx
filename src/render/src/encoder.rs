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

use draw_state::target::{Depth, Stencil};
use std::error::Error;
use std::any::Any;
use std::{fmt, mem};

use core::{Device, SubmissionResult, IndexType, Resources, VertexCount};
use core::{command, format, handle, texture};
use core::memory::{self, cast_slice, Typed, Pod, Usage};
use slice;
use pso;

/// An error occuring in memory copies.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum CopyError<S, D> {
    OutOfSrcBounds {
        size: S,
        copy_end: S,
    },
    OutOfDstBounds {
        size: D,
        copy_end: D,
    },
    Overlap {
        src_offset: usize,
        dst_offset: usize,
        size: usize,
    },
    NoSrcBindFlag,
    NoDstBindFlag,
}

/// Result type returned when copying a buffer into another buffer.
pub type CopyBufferResult = Result<(), CopyError<usize, usize>>;

/// Result type returned when copying buffer data into a texture.
pub type CopyBufferTextureResult = Result<(), CopyError<usize, [texture::Size; 3]>>;

/// Result type returned when copying texture data into a buffer.
pub type CopyTextureBufferResult = Result<(), CopyError<[texture::Size; 3], usize>>;

/// Result of copying texture to texture;
pub type CopyTextureResult = Result<(), CopyError<[texture::Size; 3], [texture::Size; 3]>>;

impl<S, D> fmt::Display for CopyError<S, D>
    where S: fmt::Debug + fmt::Display, D: fmt::Debug + fmt::Display
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CopyError::*;
        match *self {
            OutOfSrcBounds { ref size, ref copy_end } =>
                write!(f, "{}: {} / {}", self.description(), copy_end, size),
            OutOfDstBounds { ref size, ref copy_end } =>
                write!(f, "{}: {} / {}", self.description(), copy_end, size),
            Overlap { ref src_offset, ref dst_offset, ref size } =>
                write!(f, "{}: [{} - {}] to [{} - {}]",
                       self.description(),
                       src_offset, src_offset + size,
                       dst_offset, dst_offset + size),
            _ => write!(f, "{}", self.description())
        }
    }
}

impl<S, D> Error for CopyError<S, D>
    where S: fmt::Debug + fmt::Display, D: fmt::Debug + fmt::Display
{
    fn description(&self) -> &str {
        use self::CopyError::*;
        match *self {
            OutOfSrcBounds {..} => "Copy source is out of bounds",
            OutOfDstBounds {..} => "Copy destination is out of bounds",
            Overlap {..} => "Copy source and destination are overlapping",
            NoSrcBindFlag => "Copy source is missing `TRANSFER_SRC`",
            NoDstBindFlag => "Copy destination is missing `TRANSFER_DST`",
        }
    }
}

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
    InvalidUsage(Usage),
}

fn check_update_usage<T>(usage: Usage) -> Result<(), UpdateError<T>> {
    if usage == Usage::Dynamic {
        Ok(())
    } else {
        Err(UpdateError::InvalidUsage(usage))
    }
}

impl<T: Any + fmt::Debug + fmt::Display> fmt::Display for UpdateError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UpdateError::OutOfBounds {ref target, ref source} =>
                write!(f, "Write to {} from {} is out of bounds", target, source),
            UpdateError::UnitCountMismatch {ref target, ref slice} =>
                write!(f, "{}: expected {}, found {}", self.description(), target, slice),
            UpdateError::InvalidUsage(usage) =>
                write!(f, "{}: {:?}", self.description(), usage),
        }
    }
}

impl<T: Any + fmt::Debug + fmt::Display> Error for UpdateError<T> {
    fn description(&self) -> &str {
        match *self {
            UpdateError::OutOfBounds {..} => "Write to data is out of bounds",
            UpdateError::UnitCountMismatch {..} => "Unit count mismatch",
            UpdateError::InvalidUsage(_) => "This memory usage does not allow updates",
        }
    }
}

/// Graphics Command Encoder
///
/// # Overview
/// The `Encoder` is a wrapper structure around a `CommandBuffer`. It is responsible for sending
/// commands to the `CommandBuffer`.
///
/// # Construction & Handling
/// The `Encoder` implements `From<CommandBuffer>`, which is how it is constructed. There is no
/// cross-API way to create a `CommandBuffer`, however, an API back-end should expose a function to
/// create one in its `Factory` type. See the specific back-end for details on how to construct a
/// `CommandBuffer`.
///
/// The encoder exposes multiple functions that add commands to its internal `CommandBuffer`. To
/// submit these commands to the GPU so they can be rendered, call `flush`.
#[derive(Debug)]
pub struct Encoder<R: Resources, C> {
    command_buffer: C,
    raw_pso_data: pso::RawDataSet<R>,
    access_info: command::AccessInfo<R>,
    handles: handle::Manager<R>,
}

impl<R: Resources, C> From<C> for Encoder<R, C> {
    fn from(combuf: C) -> Encoder<R, C> {
        Encoder {
            command_buffer: combuf,
            raw_pso_data: pso::RawDataSet::new(),
            access_info: command::AccessInfo::new(),
            handles: handle::Manager::new(),
        }
    }
}

impl<R: Resources, C: command::Buffer<R>> Encoder<R, C> {
    /// Submits the commands in this `Encoder`'s internal `CommandBuffer` to the GPU, so they can
    /// be executed.
    ///
    /// Calling `flush` before swapping buffers is critical as without it the commands of the
    /// internal ´CommandBuffer´ will not be sent to the GPU, and as a result they will not be
    /// processed. Calling flush too often however will result in a performance hit. It is
    /// generally recommended to call flush once per frame, when all draw calls have been made.
    pub fn flush<D>(&mut self, device: &mut D)
        where D: Device<Resources=R, CommandBuffer=C>
    {
        self.flush_no_reset(device).unwrap();
        self.reset();
    }

    /// Like `flush` but keeps the encoded commands.
    pub fn flush_no_reset<D>(&mut self, device: &mut D) -> SubmissionResult<()>
        where D: Device<Resources=R, CommandBuffer=C>
    {
        device.pin_submitted_resources(&self.handles);
        device.submit(&mut self.command_buffer, &self.access_info)
    }

    /// Like `flush_no_reset` but places a fence.
    pub fn fenced_flush_no_reset<D>(&mut self,
                                    device: &mut D,
                                    after: Option<handle::Fence<R>>)
                                    -> SubmissionResult<handle::Fence<R>>
        where D: Device<Resources=R, CommandBuffer=C>
    {
        device.pin_submitted_resources(&self.handles);
        device.fenced_submit(&mut self.command_buffer, &self.access_info, after)
    }

    /// Resets the encoded commands.
    pub fn reset(&mut self) {
        self.command_buffer.reset();
        self.access_info.clear();
        self.handles.clear();
    }

    /// Copy part of a buffer to another
    pub fn copy_buffer<T: Pod>(&mut self, src: &handle::Buffer<R, T>, dst: &handle::Buffer<R, T>,
                               src_offset: usize, dst_offset: usize, size: usize) -> CopyBufferResult {
        if !src.get_info().bind.contains(memory::TRANSFER_SRC) {
            return Err(CopyError::NoSrcBindFlag);
        }
        if !dst.get_info().bind.contains(memory::TRANSFER_DST) {
            return Err(CopyError::NoDstBindFlag);
        }

        let size_bytes = mem::size_of::<T>() * size;
        let src_offset_bytes = mem::size_of::<T>() * src_offset;
        let src_copy_end = src_offset_bytes + size_bytes;
        if src_copy_end > src.get_info().size {
            return Err(CopyError::OutOfSrcBounds {
                size: src.get_info().size,
                copy_end: src_copy_end,
            });
        }
        let dst_offset_bytes = mem::size_of::<T>() * dst_offset;
        let dst_copy_end = dst_offset_bytes + size_bytes;
        if dst_copy_end > dst.get_info().size {
            return Err(CopyError::OutOfDstBounds {
                size: dst.get_info().size,
                copy_end: dst_copy_end,
            });
        }
        if src == dst &&
           src_offset_bytes < dst_copy_end &&
           dst_offset_bytes < src_copy_end
        {
            return Err(CopyError::Overlap {
                src_offset: src_offset_bytes,
                dst_offset: dst_offset_bytes,
                size: size_bytes,
            });
        }
        self.access_info.buffer_read(src.raw());
        self.access_info.buffer_write(dst.raw());

        self.command_buffer.copy_buffer(
            self.handles.ref_buffer(src.raw()).clone(),
            self.handles.ref_buffer(dst.raw()).clone(),
            src_offset_bytes, dst_offset_bytes, size_bytes);
        Ok(())
    }

    /// Copy part of a buffer to a texture
    pub fn copy_buffer_to_texture_raw(
        &mut self, src: &handle::RawBuffer<R>, src_offset_bytes: usize,
        dst: &handle::RawTexture<R>, cube_face: Option<texture::CubeFace>, info: texture::RawImageInfo)
        -> CopyBufferTextureResult
    {
        if !src.get_info().bind.contains(memory::TRANSFER_SRC) {
            return Err(CopyError::NoSrcBindFlag);
        }
        if !dst.get_info().bind.contains(memory::TRANSFER_DST) {
            return Err(CopyError::NoDstBindFlag);
        }

        let size_bytes = info.get_byte_count();
        let src_copy_end = src_offset_bytes + size_bytes;
        if src_copy_end > src.get_info().size {
            return Err(CopyError::OutOfSrcBounds {
                size: src.get_info().size,
                copy_end: src_copy_end,
            });
        }

        let dim = dst.get_info().kind.get_dimensions();
        if !info.is_inside(dim) {
            let (w, h, d, _) = dim;
            return Err(CopyError::OutOfDstBounds {
                size: [w, h, d],
                copy_end: [info.xoffset + info.width,
                           info.yoffset + info.height,
                           info.zoffset + info.depth]
            });
        }
        let dst = texture::TextureCopyRegion {
            texture: self.handles.ref_texture(dst).clone(),
            kind: dst.get_info().kind,
            cube_face,
            info,
        };

        self.access_info.buffer_read(src);

        self.command_buffer.copy_buffer_to_texture(
            self.handles.ref_buffer(src).clone(), src_offset_bytes,
            dst);
        Ok(())
    }

    /// Copy part of a texture to a buffer
    pub fn copy_texture_to_buffer_raw(
        &mut self, src: &handle::RawTexture<R>,
        cube_face: Option<texture::CubeFace>, info: texture::RawImageInfo,
        dst: &handle::RawBuffer<R>, dst_offset_bytes: usize)
        -> CopyTextureBufferResult
    {
        if !src.get_info().bind.contains(memory::TRANSFER_SRC) {
            return Err(CopyError::NoSrcBindFlag);
        }
        if !dst.get_info().bind.contains(memory::TRANSFER_DST) {
            return Err(CopyError::NoDstBindFlag);
        }

        let size_bytes = info.get_byte_count();
        let dst_copy_end = dst_offset_bytes + size_bytes;
        if dst_copy_end > dst.get_info().size {
            return Err(CopyError::OutOfDstBounds {
                size: dst.get_info().size,
                copy_end: dst_copy_end,
            });
        }

        let dim = src.get_info().kind.get_dimensions();
        if !info.is_inside(dim) {
            let (w, h, d, _) = dim;
            return Err(CopyError::OutOfSrcBounds {
                size: [w, h, d],
                copy_end: [info.xoffset + info.width,
                           info.yoffset + info.height,
                           info.zoffset + info.depth]
            });
        }
        let src = texture::TextureCopyRegion {
            texture: self.handles.ref_texture(src).clone(),
            kind: src.get_info().kind,
            cube_face,
            info,
        };

        self.access_info.buffer_write(dst);

        self.command_buffer.copy_texture_to_buffer(src,
            self.handles.ref_buffer(dst).clone(), dst_offset_bytes);
        Ok(())
    }

    /// Copy part of a texture to another texture
    pub fn copy_texture_to_texture_raw(&mut self,
        src: &handle::RawTexture<R>, src_face: Option<texture::CubeFace>, src_info: texture::RawImageInfo,
        dst: &handle::RawTexture<R>, dst_face: Option<texture::CubeFace>, dst_info: texture::RawImageInfo)
        -> CopyTextureResult
    {
        if !src.get_info().bind.contains(memory::TRANSFER_SRC) {
            return Err(CopyError::NoSrcBindFlag);
        }
        let src_dim = src.get_info().kind.get_dimensions();
        if !src_info.is_inside(src_dim) {
            let (w, h, d, _) = src_dim;
            return Err(CopyError::OutOfSrcBounds {
                size: [w, h, d],
                copy_end: [src_info.xoffset + src_info.width,
                           src_info.yoffset + src_info.height,
                           src_info.zoffset + src_info.depth]
            });
        }
        let src = texture::TextureCopyRegion {
            texture: self.handles.ref_texture(src).clone(),
            kind: src.get_info().kind,
            cube_face: src_face,
            info: src_info,
        };

        if !dst.get_info().bind.contains(memory::TRANSFER_DST) {
            return Err(CopyError::NoDstBindFlag);
        }
        let dst_dim = dst.get_info().kind.get_dimensions();
        if !dst_info.is_inside(dst_dim) {
            let (w, h, d, _) = dst_dim;
            return Err(CopyError::OutOfDstBounds {
                size: [w, h, d],
                copy_end: [dst_info.xoffset + dst_info.width,
                           dst_info.yoffset + dst_info.height,
                           dst_info.zoffset + dst_info.depth]
            });
        }
        let dst = texture::TextureCopyRegion {
            texture: self.handles.ref_texture(dst).clone(),
            kind: dst.get_info().kind,
            cube_face: dst_face,
            info: dst_info,
        };

        self.command_buffer.copy_texture_to_texture(src, dst);
        Ok(())
    }

    /// Update a buffer with a slice of data.
    pub fn update_buffer<T: Pod>(&mut self, buf: &handle::Buffer<R, T>,
                         data: &[T], offset_elements: usize)
                         -> Result<(), UpdateError<usize>>
    {
        if data.is_empty() { return Ok(()); }
        try!(check_update_usage(buf.raw().get_info().usage));

        let elem_size = mem::size_of::<T>();
        let offset_bytes = elem_size * offset_elements;
        let bound = data.len().wrapping_mul(elem_size) + offset_bytes;
        if bound <= buf.get_info().size {
            self.command_buffer.update_buffer(
                self.handles.ref_buffer(buf.raw()).clone(),
                cast_slice(data), offset_bytes);
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
        use std::slice;

        check_update_usage::<usize>(buf.raw().get_info().usage).unwrap();

        let slice = unsafe {
            slice::from_raw_parts(data as *const T as *const u8, mem::size_of::<T>())
        };
        self.command_buffer.update_buffer(
            self.handles.ref_buffer(buf.raw()).clone(), slice, 0);
    }

    /// Update the contents of a texture.
    pub fn update_texture<S, T>(&mut self, tex: &handle::Texture<R, T::Surface>,
                          cube_face: Option<texture::CubeFace>,
                          img: texture::NewImageInfo, data: &[S::DataType])
                          -> Result<(), UpdateError<[texture::Size; 3]>>
    where
        S: format::SurfaceTyped,
        S::DataType: Copy,
        T: format::Formatted<Surface = S>,
    {
        if data.is_empty() { return Ok(()); }
        try!(check_update_usage(tex.raw().get_info().usage));

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
        let dst = texture::TextureCopyRegion {
            texture: self.handles.ref_texture(tex.raw()).clone(),
            kind: tex.get_info().kind,
            cube_face,
            info: img.convert(T::get_format()),
        };

        self.command_buffer.update_texture(dst, cast_slice(data));
        Ok(())
    }

    fn draw_indexed<T>(&mut self, buf: &handle::Buffer<R, T>, ty: IndexType,
                    slice: &slice::Slice<R>, base: VertexCount,
                    instances: Option<command::InstanceParams>) {
        self.access_info.buffer_read(buf.raw());
        self.command_buffer.bind_index(self.handles.ref_buffer(buf.raw()).clone(), ty);
        self.command_buffer.call_draw_indexed(slice.start, slice.end - slice.start, base, instances);
    }

    fn draw_slice(&mut self, slice: &slice::Slice<R>, instances: Option<command::InstanceParams>) {
        match slice.buffer {
            slice::IndexBuffer::Auto => self.command_buffer.call_draw(
                slice.start + slice.base_vertex, slice.end - slice.start, instances),
            slice::IndexBuffer::Index16(ref buf) =>
                self.draw_indexed(buf, IndexType::U16, slice, slice.base_vertex, instances),
            slice::IndexBuffer::Index32(ref buf) =>
                self.draw_indexed(buf, IndexType::U32, slice, slice.base_vertex, instances),
        }
    }

    /// Clears the supplied `RenderTargetView` to the supplied `ClearColor`.
    pub fn clear<T: format::RenderFormat>(&mut self,
                 view: &handle::RenderTargetView<R, T>, value: T::View)
    where T::View: Into<command::ClearColor> {
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

    /// Draws a `slice::Slice` using a pipeline state object, and its matching `Data` structure.
    pub fn draw<D: pso::PipelineData<R>>(&mut self, slice: &slice::Slice<R>,
                pipeline: &pso::PipelineState<R, D::Meta>, user_data: &D)
    {
        let (pso, _) = self.handles.ref_pso(pipeline.get_handle());
        //TODO: make `raw_data` a member to this struct, to re-use the heap allocation
        self.raw_pso_data.clear();
        user_data.bake_to(&mut self.raw_pso_data, pipeline.get_meta(), &mut self.handles, &mut self.access_info);
        self.command_buffer.bind_pixel_targets(self.raw_pso_data.pixel_targets.clone());
        self.command_buffer.bind_pipeline_state(pso.clone());
        self.command_buffer.bind_vertex_buffers(self.raw_pso_data.vertex_buffers.clone());
        self.command_buffer.set_ref_values(self.raw_pso_data.ref_values);
        self.command_buffer.set_scissor(self.raw_pso_data.scissor);
        self.command_buffer.bind_constant_buffers(&self.raw_pso_data.constant_buffers);
        for &(location, value) in &self.raw_pso_data.global_constants {
            self.command_buffer.bind_global_constant(location, value);
        }
        self.command_buffer.bind_unordered_views(&self.raw_pso_data.unordered_views);
        //Note: it's important to bind RTV, DSV, and UAV before SRV
        self.command_buffer.bind_resource_views(&self.raw_pso_data.resource_views);
        self.command_buffer.bind_samplers(&self.raw_pso_data.samplers);
        self.draw_slice(slice, slice.instances);
    }

    /// Generate a mipmap chain for the given resource view.
    pub fn generate_mipmap<T: format::BlendFormat>(&mut self, view: &handle::ShaderResourceView<R, T>) {
        self.generate_mipmap_raw(view.raw())
    }

    /// Untyped version of mipmap generation.
    pub fn generate_mipmap_raw(&mut self, view: &handle::RawShaderResourceView<R>) {
        let srv = self.handles.ref_srv(view).clone();
        self.command_buffer.generate_mipmap(srv);
    }
}
