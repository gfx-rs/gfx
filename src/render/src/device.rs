use std::mem;
use std::ops::Range;
use core::{Device as CoreDevice, HeapType};
use core::device::TargetViewError;

use memory::{self, Typed, Pod};
use handle::{self, GarbageSender};
use handle::inner::*;
use {buffer, image, format};
use Backend;

/*
/// Error creating a PipelineState
#[derive(Clone, PartialEq, Debug)]
pub enum PipelineStateError<S> {
    /// Shader program failed to link.
    Program(ProgramError),
    /// Unable to create PSO descriptor due to mismatched formats.
    DescriptorInit(pso::InitError<S>),
    /// Device failed to create the handle give the descriptor.
    DeviceCreate(CreationError),
}

impl<'a> From<PipelineStateError<&'a str>> for PipelineStateError<String> {
    fn from(pse: PipelineStateError<&'a str>) -> PipelineStateError<String> {
        match pse {
            PipelineStateError::Program(e) => PipelineStateError::Program(e),
            PipelineStateError::DescriptorInit(e) => PipelineStateError::DescriptorInit(e.into()),
            PipelineStateError::DeviceCreate(e) => PipelineStateError::DeviceCreate(e),
        }
    }
}

impl<S: fmt::Debug + fmt::Display> fmt::Display for PipelineStateError<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PipelineStateError::Program(ref e) => write!(f, "{}: {}", self.description(), e),
            PipelineStateError::DescriptorInit(ref e) => write!(f, "{}: {}", self.description(), e),
            PipelineStateError::DeviceCreate(ref e) => write!(f, "{}: {}", self.description(), e),
        }
    }
}

impl<S: fmt::Debug + fmt::Display> Error for PipelineStateError<S> {
    fn description(&self) -> &str {
        match *self {
            PipelineStateError::Program(_) => "Shader program failed to link",
            PipelineStateError::DescriptorInit(_) =>
                "Unable to create PSO descriptor due to mismatched formats",
            PipelineStateError::DeviceCreate(_) => "Device failed to create the handle give the descriptor",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            PipelineStateError::Program(ref program_error) => Some(program_error),
            PipelineStateError::DescriptorInit(ref init_error) => Some(init_error),
            PipelineStateError::DeviceCreate(ref creation_error) => Some(creation_error),
        }
    }
}

impl<S> From<ProgramError> for PipelineStateError<S> {
    fn from(e: ProgramError) -> Self {
        PipelineStateError::Program(e)
    }
}

impl<S> From<pso::InitError<S>> for PipelineStateError<S> {
    fn from(e: pso::InitError<S>) -> Self {
        PipelineStateError::DescriptorInit(e)
    }
}

impl<S> From<CreationError> for PipelineStateError<S> {
    fn from(e: CreationError) -> Self {
        PipelineStateError::DeviceCreate(e)
    }
}

/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
    /// The requested mapping access overlaps with another.
    AccessOverlap,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            InvalidAccess(ref access, ref usage) => {
                write!(f, "{}: access = {:?}, usage = {:?}", self.description(), access, usage)
            }
            AccessOverlap => write!(f, "{}", self.description())
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            InvalidAccess(..) => "The requested mapping access did not match the expected usage",
            AccessOverlap => "The requested mapping access overlaps with another"
        }
    }
}
*/

#[derive(Clone)]
pub struct Device<B: Backend> {
    raw: B::Device,
    // TODO: could be shared instead of cloned
    heap_types: Vec<HeapType>,
    memory_heaps: Vec<u64>,
    garbage: GarbageSender<B>,
}

impl<B: Backend> Device<B> {
    pub(crate) fn new(
        raw: B::Device,
        heap_types: Vec<HeapType>,
        memory_heaps: Vec<u64>,
        garbage: GarbageSender<B>) -> Self
    {
        Device { raw, heap_types, memory_heaps, garbage }
    }

    // TODO: remove
    pub fn heap_types(&self) -> &[HeapType] {
        &self.heap_types
    }

    // TODO: remove
    pub fn memory_heaps(&self) -> &[u64] {
        &self.memory_heaps
    }

    pub fn ref_raw(&self) -> &B::Device {
        &self.raw
    }

    pub fn mut_raw(&mut self) -> &mut B::Device {
        &mut self.raw
    }

    #[allow(unused_variables)]
    pub fn create_buffer_raw(&mut self,
        role: buffer::Role,
        usage: memory::Usage,
        bind: memory::Bind,
        size: usize,
        stride: usize
    ) -> Result<handle::raw::Buffer<B>, buffer::CreationError>
    {
        unimplemented!()
    }

    pub fn create_buffer<T: Pod>(&mut self,
        role: buffer::Role,
        usage: memory::Usage,
        bind: memory::Bind,
        size: usize
    ) -> Result<handle::Buffer<B, T>, buffer::CreationError>
    {
        let stride = mem::size_of::<T>();
        self.create_buffer_raw(
            role,
            usage,
            bind,
            size * stride,
            stride
        ).map(Typed::new)
    }

    #[allow(unused_variables)]
    pub fn create_image_raw(&mut self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        bind: memory::Bind,
        usage: memory::Usage
    ) -> Result<handle::raw::Image<B>, image::CreationError>
    {
        unimplemented!()
    }

    pub fn create_image<F>(&mut self,
        kind: image::Kind,
        mip_levels: image::Level,
        bind: memory::Bind,
        usage: memory::Usage
    ) -> Result<handle::Image<B, F>, image::CreationError>
        where F: format::Formatted
    {
        self.create_image_raw(
            kind,
            mip_levels,
            F::get_format(),
            bind,
            usage
        ).map(Typed::new)
    }

    pub fn create_sampler(&mut self, info: image::SamplerInfo)
        -> handle::Sampler<B>
    {
        handle::inner::Sampler::new(
            self.raw.create_sampler(info.clone()), info, self.garbage.clone()
        ).into()
    }

    pub fn view_buffer_as_constant_raw(&mut self,
        buffer: &handle::raw::Buffer<B>,
        range: Range<u64>,
    ) -> Result<handle::raw::ConstantBufferView<B>, TargetViewError>
    {
        self.raw.view_buffer_as_constant(buffer.resource(), range)
            .map(|cbv| ConstantBufferView::new(
                cbv,
                buffer.clone(),
                self.garbage.clone()
            ).into())
    }

    pub fn view_buffer_as_constant<T>(&mut self,
        buffer: &handle::Buffer<B, T>,
        range: Range<u64>,
    ) -> Result<handle::ConstantBufferView<B, T>, TargetViewError>
    {
        self.view_buffer_as_constant_raw(buffer, range)
            .map(Typed::new)
    }

    pub(crate) fn view_backbuffer_as_render_target_raw(&mut self,
        image: B::Image,
        kind: image::Kind,
        format: format::Format,
        range: image::SubresourceRange
    ) -> Result<handle::raw::RenderTargetView<B>, TargetViewError> {
        let info = image::Info {
            kind,
            levels: 1,
            format: format.0,
            bind: memory::RENDER_TARGET | memory::TRANSFER_SRC,
            usage: memory::Usage::Data,
        };
        self.raw.view_image_as_render_target(&image, format, range)
            .map(|rtv| RenderTargetView::new(
                rtv,
                handle::ViewSource::Backbuffer(image, info),
                self.garbage.clone()
            ).into())
    }

    // TODO
    // pub(crate) fn view_backbuffer_as_depth_stencil_raw
    // pub fn view_image_as_depth_stencil_raw
    // pub fn view_image_as_depth_stencil

    pub fn view_image_as_render_target_raw(&mut self,
        image: &handle::raw::Image<B>,
        format: format::Format,
        range: image::SubresourceRange
    ) -> Result<handle::raw::RenderTargetView<B>, TargetViewError>
    {
        self.raw.view_image_as_render_target(image.resource(), format, range)
            .map(|rtv| RenderTargetView::new(
                rtv,
                image.into(),
                self.garbage.clone()
            ).into())
    }

    pub fn view_image_as_render_target<F>(&mut self,
        image: &handle::Image<B, F>,
        range: image::SubresourceRange
    ) -> Result<handle::RenderTargetView<B, F>, TargetViewError>
        where F: format::RenderFormat
    {
        self.view_image_as_render_target_raw(image, F::get_format(), range)
            .map(Typed::new)
    }
    
    pub fn view_image_as_shader_resource_raw(&mut self,
        image: &handle::raw::Image<B>,
        format: format::Format
    ) -> Result<handle::raw::ShaderResourceView<B>, TargetViewError>
    {
        self.raw.view_image_as_shader_resource(image.resource(), format)
            .map(|srv| ShaderResourceView::new(
                srv,
                image.into(),
                self.garbage.clone()
            ).into())
    }

    // TODO: rename to simply ViewError ?
    pub fn view_image_as_shader_resource<F>(&mut self, image: &handle::Image<B, F>)
        -> Result<handle::ShaderResourceView<B, F>, TargetViewError>
        where F: format::ImageFormat
    {
        self.view_image_as_shader_resource_raw(image, F::get_format())
            .map(Typed::new)
    }

    pub fn view_image_as_unordered_access_raw(&mut self,
        image: &handle::raw::Image<B>,
        format: format::Format
    ) -> Result<handle::raw::UnorderedAccessView<B>, TargetViewError>
    {
        self.raw.view_image_as_unordered_access(image.resource(), format)
            .map(|uav| UnorderedAccessView::new(
                uav,
                image.into(),
                self.garbage.clone()
            ).into())
    }

    pub fn view_image_as_unordered_access<F>(&mut self, image: &handle::Image<B, F>)
        -> Result<handle::UnorderedAccessView<B, F>, TargetViewError>
        where F: format::ImageFormat
    {
        self.view_image_as_unordered_access_raw(image, F::get_format())
            .map(Typed::new)
    }

/*
    /// Creates an immutable vertex buffer from the supplied vertices.
    /// A `Slice` will have to manually be constructed.
    fn create_vertex_buffer<T>(&mut self, vertices: &[T])
                               -> handle::Buffer<B, T>
        where T: Pod + pso::buffer::Structure<format::Format>
    {
        //debug_assert!(nv <= self.get_capabilities().max_vertex_count);
        self.create_buffer_immutable(vertices, buffer::Role::Vertex, Bind::empty())
            .unwrap()
    }

    /// Creates an immutable index buffer from the supplied vertices.
    ///
    /// The paramater `indices` is typically a &[u16] or &[u32] slice.
    fn create_index_buffer<T>(&mut self, indices: T)
                              -> IndexBuffer<B>
        where T: IntoIndexBuffer<B>
    {
        indices.into_index_buffer(self)
    }

    /// Creates an immutable vertex buffer from the supplied vertices,
    /// together with a `Slice` from the supplied indices.
    fn create_vertex_buffer_with_slice<I, V>(&mut self, vertices: &[V], indices: I)
                                             -> (handle::Buffer<B, V>, Slice<B>)
        where V: Pod + pso::buffer::Structure<format::Format>,
              I: IntoIndexBuffer<B>
    {
        let vertex_buffer = self.create_vertex_buffer(vertices);
        let index_buffer = self.create_index_buffer(indices);
        let buffer_length = match index_buffer {
            IndexBuffer::Auto => vertex_buffer.len(),
            IndexBuffer::Index16(ref ib) => ib.len(),
            IndexBuffer::Index32(ref ib) => ib.len(),
        };

        (vertex_buffer, Slice {
            start: 0,
            end: buffer_length as u32,
            base_vertex: 0,
            instances: None,
            buffer: index_buffer
        })
    }

    /// Creates a constant buffer for `num` identical elements of type `T`.
    fn create_constant_buffer<T>(&mut self, num: usize) -> handle::Buffer<B, T>
        where T: Copy
    {
        self.create_buffer(num,
                           buffer::Role::Constant,
                           memory::Usage::Dynamic,
                           Bind::empty()).unwrap()
    }

    /// Creates an upload buffer for `num` elements of type `T`.
    fn create_upload_buffer<T>(&mut self, num: usize)
                               -> Result<handle::Buffer<B, T>, buffer::CreationError>
    {
        self.create_buffer(num,
                           buffer::Role::Staging,
                           memory::Usage::Upload,
                           memory::TRANSFER_SRC)
    }

    /// Creates a download buffer for `num` elements of type `T`.
    fn create_download_buffer<T>(&mut self, num: usize)
                                 -> Result<handle::Buffer<B, T>, buffer::CreationError>
    {
        self.create_buffer(num,
                           buffer::Role::Staging,
                           memory::Usage::Download,
                           memory::TRANSFER_DST)
    }

    /// Creates a `ShaderSet` from the supplied vertex and pixel shader source code.
    fn create_shader_set(&mut self, vs_code: &[u8], ps_code: &[u8])
                         -> Result<ShaderSet<B>, ProgramError> {
        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };
        Ok(ShaderSet::Simple(vs, ps))
    }

    /// Creates a `ShaderSet` from the supplied vertex, geometry, and pixel
    /// shader source code. Mainly used for testing.
    fn create_shader_set_geometry(&mut self, vs_code: &[u8], gs_code: &[u8], ps_code: &[u8])
                         -> Result<ShaderSet<B>, ProgramError> {
        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let gs = match self.create_shader_geometry(gs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Geometry(e)),
        };
        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };
        Ok(ShaderSet::Geometry(vs, gs, ps))
    }

    /// Creates a `ShaderSet` from the supplied vertex, hull, domain, and pixel
    /// shader source code. Mainly used for testing.
    fn create_shader_set_tessellation(&mut self, vs_code: &[u8], hs_code: &[u8], ds_code: &[u8], ps_code: &[u8])
                         -> Result<ShaderSet<B>, ProgramError> {
        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };

        let hs = match self.create_shader_hull(hs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Hull(e)),
        };

        let ds = match self.create_shader_domain(ds_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Domain(e)),
        };

        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };
        Ok(ShaderSet::Tessellated(vs, hs, ds, ps))
    }

    /// Creates a basic shader `Program` from the supplied vertex and pixel shader source code.
    fn link_program(&mut self, vs_code: &[u8], ps_code: &[u8])
                    -> Result<handle::Program<B>, ProgramError> {

        let set = try!(self.create_shader_set(vs_code, ps_code));
        self.create_program(&set).map_err(|e| ProgramError::Link(e))
    }

    /// Similar to `create_pipeline_from_program(..)`, but takes a `ShaderSet` as opposed to a
    /// shader `Program`.
    fn create_pipeline_state<I: pso::PipelineInit>(&mut self, shaders: &ShaderSet<B>,
                             primitive: Primitive, rasterizer: state::Rasterizer, init: I)
                             -> Result<pso::PipelineState<B, I::Meta>, PipelineStateError<String>>
    {
        let program = try!(self.create_program(shaders).map_err(|e| ProgramError::Link(e)));
        self.create_pipeline_from_program(&program, primitive, rasterizer, init).map_err(|error| {
            use self::PipelineStateError::*;
            match error {
                Program(e) => Program(e),
                DescriptorInit(e) => DescriptorInit(e.into()),
                DeviceCreate(e) => DeviceCreate(e),
            }
        })
    }

    /// Creates a strongly typed `PipelineState` from its `Init` structure, a shader `Program`, a
    /// primitive type and a `Rasterizer`.
    fn create_pipeline_from_program<'a, I: pso::PipelineInit>(&mut self, program: &'a handle::Program<B>,
                                    primitive: Primitive, rasterizer: state::Rasterizer, init: I)
                                    -> Result<pso::PipelineState<B, I::Meta>, PipelineStateError<&'a str>>
    {
        let mut descriptor = Descriptor::new(primitive, rasterizer);
        let meta = try!(init.link_to(&mut descriptor, program.get_info()));
        let raw = try!(self.create_pipeline_state_raw(program, &descriptor));

        Ok(pso::PipelineState::new(raw, primitive, meta))
    }

    /// Creates a strongly typed `PipelineState` from its `Init` structure. Automatically creates a
    /// shader `Program` from a vertex and pixel shader source, as well as a `Rasterizer` capable
    /// of rendering triangle faces without culling.
    fn create_pipeline_simple<I: pso::PipelineInit>(&mut self, vs: &[u8], ps: &[u8], init: I)
                              -> Result<pso::PipelineState<B, I::Meta>, PipelineStateError<String>>
    {
        let set = try!(self.create_shader_set(vs, ps));
        self.create_pipeline_state(&set, Primitive::TriangleList, state::Rasterizer::new_fill(),
                                   init)
    }

    /// Create a linear sampler with clamping to border.
    fn create_sampler_linear(&mut self) -> handle::Sampler<B> {
        self.create_sampler(texture::SamplerInfo::new(
            texture::FilterMethod::Trilinear,
            texture::WrapMode::Clamp,
        ))
    }
    */
}
