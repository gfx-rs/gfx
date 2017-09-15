//! Commands encoder.

use std::error::Error;
use std::any::Any;
use std::fmt;
use std::sync::mpsc;
use std::collections::HashSet;

use core::{self, CommandPool};
use core::command::CommandBuffer;
use memory::{Usage, Provider, Dependency};
use {handle, image, Backend};

pub struct Pool<B: Backend, C>(Provider<PoolInner<B, C>>);

#[derive(Clone)]
pub(crate) struct PoolDependency<B: Backend, C>(Dependency<PoolInner<B, C>>);

impl<B: Backend, C> Pool<B, C> {
    pub(crate) fn new(
        inner: CommandPool<B, C>,
        sender: CommandPoolSender<B, C>
    ) -> Self {
        Pool(Provider::new(PoolInner { inner: Some(inner), sender }))
    }

    fn mut_inner<'a>(&'a mut self) -> &'a mut CommandPool<B, C> {
        self.0.get_mut()
    }

    pub fn reserve(&mut self, additional: usize) {
        self.mut_inner().reserve(additional);
    }

    pub fn acquire_encoder<'a>(&'a mut self) -> Encoder<'a, B, C> {
        Encoder {
            pool: PoolDependency(self.0.dependency()),
            buffer: self.mut_inner().acquire_command_buffer(),
            // raw_data: pso::RawDataSet::new(),
            access_info: AccessInfo::new(),
            handles: handle::Bag::new(),
        }
    }
}

struct PoolInner<B: Backend, C> {
    // option for owned drop
    inner: Option<CommandPool<B, C>>,
    sender: CommandPoolSender<B, C>,
}

impl<B: Backend, C> PoolInner<B, C> {
    fn get_mut(&mut self) -> &mut CommandPool<B, C> {
        self.inner.as_mut().unwrap()
    }
}

impl<B: Backend, C> Drop for PoolInner<B, C> {
    fn drop(&mut self) {
        // simply will not be recycled if the channel is down, should be ok.
        let _ = self.sender.send(self.inner.take().unwrap());
    }
}

pub(crate) type CommandPoolSender<B, C> = mpsc::Sender<CommandPool<B, C>>;
pub(crate) type CommandPoolReceiver<B, C> = mpsc::Receiver<CommandPool<B, C>>;
pub(crate) fn command_pool_channel<B: Backend, C>()
    -> (CommandPoolSender<B, C>, CommandPoolReceiver<B, C>) {
    mpsc::channel()
}

pub struct Encoder<'a, B: Backend, C> {
    buffer: CommandBuffer<'a, B, C>,
    // raw_data: pso::RawDataSet<B>,
    access_info: AccessInfo<B>,
    handles: handle::Bag<B>,
    pool: PoolDependency<B, C>
}

pub struct Submit<B: Backend, C> {
    pub(crate) inner: core::command::Submit<B, C>,
    pub(crate) access_info: AccessInfo<B>,
    pub(crate) handles: handle::Bag<B>,
    pub(crate) pool: PoolDependency<B, C>
}

/// Informations about what is accessed by a submit.
#[derive(Debug)]
pub struct AccessInfo<B: Backend> {
    buffers: HashSet<handle::raw::Buffer<B>>,
}

impl<B: Backend> AccessInfo<B> {
    /// Creates empty access informations
    pub fn new() -> Self {
        AccessInfo {
            buffers: HashSet::new(),
        }
    }

    /// Clear access informations
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    /// Register a buffer read access
    pub fn buffer_read(&mut self, buffer: handle::raw::Buffer<B>) {
        self.buffers.insert(buffer);
    }

    /// Register a buffer write access
    pub fn buffer_write(&mut self, buffer: handle::raw::Buffer<B>) {
        self.buffers.insert(buffer);
    }

    pub fn append(&mut self, other: &mut AccessInfo<B>) {
        self.buffers.extend(other.buffers.drain());
    }

    pub(crate) fn acquire_accesses(&self) {
        for buffer in &self.buffers {
            assert!(buffer.info().acquire_access());
        }
    }

    pub(crate) fn release_accesses(&self) {
        for buffer in &self.buffers {
            buffer.info().release_access();
        }
    }
}

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

/// Result type returned when copying buffer data into an image.
pub type CopyBufferImageResult = Result<(), CopyError<usize, [image::Size; 3]>>;

/// Result type returned when copying image data into a buffer.
pub type CopyImageBufferResult = Result<(), CopyError<[image::Size; 3], usize>>;

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

/// An error occuring in buffer and image updates.
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
    if usage == Usage::Data {
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

impl<B: Backend, C> Submit<B, C> {
    /*
    /// Submits the commands in the internal `CommandBuffer` to the GPU, so they can
    /// be executed.
    pub fn synced_flush(self,
                        queue: &mut GraphicsQueue<B>,
                        wait_semaphores: &[&handle::Semaphore<B>],
                        signal_semaphores: &[&handle::Semaphore<B>],
                        fence: Option<&handle::Fence<B>>) -> SubmissionResult<()> {
        let wait_semaphores = &wait_semaphores.iter()
                                              .map(|&wait| (wait, core::pso::BOTTOM_OF_PIPE))
                                              .collect::<Vec<_>>();
        queue.pin_submitted_resources(&self.handles);
        let submission =
            core::Submission::new()
                    .wait_on(wait_semaphores)
                    .submit(&[self.submission])
                    .signal(signal_semaphores);

        queue.submit(
            &[submission],
            fence,
            &self.access_info
        );

        Ok(()) // TODO
    }
    */
}

impl<'a, B: Backend, C> Encoder<'a, B, C> {
    pub fn mut_buffer(&mut self) -> &mut CommandBuffer<'a, B, C> {
        &mut self.buffer
    }

    pub fn finish(self) -> Submit<B, C> {
        Submit {
            inner: self.buffer.finish(),
            access_info: self.access_info,
            handles: self.handles,
            pool: self.pool,
        }
    }

    /*
    /// Submits the internal `CommandBuffer` to the GPU, so it can be executed.
    ///
    /// Calling `flush` before swapping buffers is critical as without it the commands of the
    /// internal ´CommandBuffer´ will not be sent to the GPU, and as a result they will not be
    /// processed. Calling flush too often however will result in a performance hit. It is
    /// generally recommended to call flush once per frame, when all draw calls have been made.
    pub fn flush(self, queue: &mut GraphicsQueue<B>) -> SubmissionResult<()> {
        self.synced_flush(queue, &[], &[], None)
    }

    /// Submits the commands in the internal `CommandBuffer` to the GPU, so they can
    /// be executed.
    pub fn synced_flush(self,
                        queue: &mut GraphicsQueue<B>,
                        wait_semaphores: &[&handle::Semaphore<B>],
                        signal_semaphores: &[&handle::Semaphore<B>],
                        fence: Option<&handle::Fence<B>>) -> SubmissionResult<()> {
        self.finish().synced_flush(queue, wait_semaphores, signal_semaphores, fence)
    }

    /// Copy part of a buffer to another
    pub fn copy_buffer<T: Pod>(&mut self, src: &handle::Buffer<B, T>, dst: &handle::Buffer<B, T>,
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
        &mut self, src: &handle::RawBuffer<B>, src_offset_bytes: usize,
        dst: &handle::RawTexture<B>, face: Option<texture::CubeFace>, info: texture::RawImageInfo)
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

        self.access_info.buffer_read(src);

        self.command_buffer.copy_buffer_to_texture(
            self.handles.ref_buffer(src).clone(), src_offset_bytes,
            self.handles.ref_texture(dst).clone(), dst.get_info().kind,
            face, info);
        Ok(())
    }

    /// Copy part of a texture to a buffer
    pub fn copy_texture_to_buffer_raw(
        &mut self, src: &handle::RawTexture<B>,
        face: Option<texture::CubeFace>, info: texture::RawImageInfo,
        dst: &handle::RawBuffer<B>, dst_offset_bytes: usize)
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

        self.access_info.buffer_write(dst);

        self.command_buffer.copy_texture_to_buffer(
            self.handles.ref_texture(src).clone(), src.get_info().kind,
            face, info,
            self.handles.ref_buffer(dst).clone(), dst_offset_bytes);
        Ok(())
    }

    /// Update a buffer with a slice of data.
    pub fn update_buffer<T: Pod>(&mut self, buf: &handle::Buffer<B, T>,
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
    pub fn update_constant_buffer<T: Copy>(&mut self, buf: &handle::Buffer<B, T>, data: &T) {
        use std::slice;

        check_update_usage::<usize>(buf.raw().get_info().usage).unwrap();

        let slice = unsafe {
            slice::from_raw_parts(data as *const T as *const u8, mem::size_of::<T>())
        };
        self.command_buffer.update_buffer(
            self.handles.ref_buffer(buf.raw()).clone(), slice, 0);
    }

    /// Update the contents of a texture.
    pub fn update_texture<S, T>(&mut self, tex: &handle::Texture<B, T::Surface>,
                          face: Option<texture::CubeFace>,
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

        self.command_buffer.update_texture(
            self.handles.ref_texture(tex.raw()).clone(),
            tex.get_info().kind, face, cast_slice(data),
            img.convert(T::get_format()));
        Ok(())
    }

    fn draw_indexed<T>(&mut self, buf: &handle::Buffer<B, T>, ty: IndexType,
                    slice: &slice::Slice<B>, base: VertexCount,
                    instances: Option<command::InstanceParams>) {
        self.access_info.buffer_read(buf.raw());
        self.command_buffer.bind_index(self.handles.ref_buffer(buf.raw()).clone(), ty);
        self.command_buffer.call_draw_indexed(slice.start, slice.end - slice.start, base, instances);
    }

    fn draw_slice(&mut self, slice: &slice::Slice<B>, instances: Option<command::InstanceParams>) {
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
                 view: &handle::RenderTargetView<B, T>, value: T::View)
    where T::View: Into<command::ClearColor> {
        let target = self.handles.ref_rtv(view.raw()).clone();
        self.command_buffer.clear_color(target, value.into())
    }
    /// Clear a depth view with a specified value.
    pub fn clear_depth<T: format::DepthFormat>(&mut self,
                       view: &handle::DepthStencilView<B, T>, depth: Depth) {
        let target = self.handles.ref_dsv(view.raw()).clone();
        self.command_buffer.clear_depth_stencil(target, Some(depth), None)
    }

    /// Clear a stencil view with a specified value.
    pub fn clear_stencil<T: format::StencilFormat>(&mut self,
                         view: &handle::DepthStencilView<B, T>, stencil: Stencil) {
        let target = self.handles.ref_dsv(view.raw()).clone();
        self.command_buffer.clear_depth_stencil(target, None, Some(stencil))
    }

    /// Draws a `slice::Slice` using a pipeline state object, and its matching `Data` structure.
    pub fn draw<D: pso::PipelineData<B>>(&mut self, slice: &slice::Slice<B>,
                pipeline: &pso::PipelineState<B, D::Meta>, user_data: &D)
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
    pub fn generate_mipmap<T: format::BlendFormat>(&mut self, view: &handle::ShaderResourceView<B, T>) {
        self.generate_mipmap_raw(view.raw())
    }

    /// Untyped version of mipmap generation.
    pub fn generate_mipmap_raw(&mut self, view: &handle::RawShaderResourceView<B>) {
        let srv = self.handles.ref_srv(view).clone();
        self.command_buffer.generate_mipmap(srv);
    }
    */
}
