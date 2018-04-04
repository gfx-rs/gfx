use std::mem;
use std::ops::Range;
use std::sync::Arc;

use hal::{self, Device as CoreDevice, MemoryType, MemoryTypeId};
use hal::memory::{Properties};

use memory::{self, Allocator, Typed};
use handle::{self, GarbageSender};
use handle::inner::*;
use {buffer, image, format, pso};
use {Backend, Primitive};

pub use hal::device::{FramebufferError};

pub struct Device<B: Backend> {
    pub raw: Arc<B::Device>,
    // TODO: could be shared instead of cloned
    memory_types: Vec<MemoryType>,
    memory_heaps: Vec<u64>,
    garbage: GarbageSender<B>,
}

impl<B: Backend> Clone for Device<B> {
    fn clone(&self) -> Self {
        Device {
            raw: self.raw.clone(),
            memory_types: self.memory_types.clone(),
            memory_heaps: self.memory_heaps.clone(),
            garbage: self.garbage.clone(),
        }
    }
}

pub struct InitToken<B: Backend> {
    pub(crate) handle: handle::Any<B>,
}

impl<B: Backend> Device<B> {
    pub(crate) fn new(
        raw: B::Device,
        memory_types: Vec<MemoryType>,
        memory_heaps: Vec<u64>,
    ) -> (Self, handle::GarbageCollector<B>)
    {
        let arc = Arc::new(raw);
        let (garbage, collector) = handle::garbage(&arc);
        (Device { raw: arc, memory_types, memory_heaps, garbage }, collector)
    }

    pub fn memory_types(&self) -> &[MemoryType] {
        &self.memory_types
    }

    pub fn memory_heaps(&self) -> &[u64] {
        &self.memory_heaps
    }

    pub fn find_memory<P>(&self, type_mask: u64, predicate: P) -> Option<MemoryTypeId>
        where P: Fn(Properties) -> bool
    {
        self.memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                type_mask & (1 << id) != 0 &&
                predicate(memory_type.properties)
            })
            .map(Into::into)
    }

    pub fn find_usage_memory(&self, usage: memory::Usage, type_mask: u64) -> Option<MemoryTypeId> {
        use memory::Usage::*;
        match usage {
            Data => self.find_data_memory(type_mask),
            Upload => self.find_upload_memory(type_mask),
            Download => self.find_download_memory(type_mask),
        }
    }

    // TODO: fallbacks when out of memory

    pub fn find_data_memory(&self, type_mask: u64) -> Option<MemoryTypeId> {
        self.find_memory(type_mask, |props| {
            props.contains(Properties::DEVICE_LOCAL) && !props.contains(Properties::CPU_VISIBLE)
        }).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(Properties::DEVICE_LOCAL)
        }))
    }

    pub fn find_upload_memory(&self, type_mask: u64) -> Option<MemoryTypeId> {
        self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE | Properties::COHERENT)
            && !props.contains(Properties::CPU_CACHED)
        }).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE | Properties::COHERENT)
        })).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE)
        }))
    }

    pub fn find_download_memory(&self, type_mask: u64) -> Option<MemoryTypeId> {
        self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED)
        }).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE | Properties::CPU_CACHED)
        })).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE | Properties::COHERENT)
        })).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(Properties::CPU_VISIBLE)
        }))
    }

    pub fn create_buffer_raw<A>(
        &mut self,
        allocator: &mut A,
        usage: buffer::Usage,
        size: u64,
        stride: u64
    ) -> Result<(handle::raw::Buffer<B>, InitToken<B>), buffer::CreationError>
        where A: Allocator<B>
    {
        let buffer = self.raw.create_buffer(size, usage)?;
        let (buffer, memory) = allocator.allocate_buffer(self, usage, buffer);
        let info = buffer::Info::new(usage, memory, size, stride);
        let handle = handle::raw::Buffer::from(
            Buffer::new(buffer, info, self.garbage.clone()));
        let token = InitToken { handle: handle.clone().into() };
        Ok((handle, token))
    }

    pub fn create_buffer<T, A>(
        &mut self,
        allocator: &mut A,
        usage: buffer::Usage,
        size: u64
    ) -> Result<(handle::Buffer<B, T>, InitToken<B>), buffer::CreationError>
        where T: Copy, A: Allocator<B>
    {
        let stride = mem::size_of::<T>() as u64;
        self.create_buffer_raw(
            allocator,
            usage,
            size * stride,
            stride
        ).map(|(h, t)| (Typed::new(h), t))
    }

    pub fn create_buffer_view_raw(
        &mut self,
        buffer: &handle::raw::Buffer<B>,
        format: Option<format::Format>,
        range: Range<u64>,
    ) -> Result<handle::raw::BufferView<B>, buffer::ViewError> {
        self.raw.create_buffer_view(buffer.resource(), format, range)
            .map(|view| BufferView::new(
                view,
                buffer.clone(),
                self.garbage.clone()
            ).into())
    }

    pub fn create_buffer_view<T>(
        &mut self,
        buffer: &handle::Buffer<B, T>,
        format: Option<format::Format>,
        range: Range<u64>,
    ) -> Result<handle::BufferView<B, T>, buffer::ViewError> {
        self.create_buffer_view_raw(buffer.as_ref(), format, range)
            .map(Typed::new)
    }

    pub fn create_image_raw<A: Allocator<B>>(
        &mut self,
        allocator: &mut A,
        usage: image::Usage,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
    ) -> Result<(handle::raw::Image<B>, InitToken<B>), image::CreationError>
        where A: Allocator<B>
    {
        use image::Usage;
        use hal::image::{Layout, Tiling};

        let aspects = format.aspects();
        let flags = image::StorageFlags::empty();
        let image = self.raw.create_image(kind, mip_levels, format, Tiling::Optimal, usage, flags)?;
        let (image, memory) = allocator.allocate_image(self, usage, image);
        let origin = image::Origin::User(memory);
        let stable_access = hal::image::Access::empty();
        let stable_layout = match usage {
            _ if usage.contains(Usage::COLOR_ATTACHMENT) =>
                Layout::ColorAttachmentOptimal,
            _ if usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) =>
                Layout::DepthStencilAttachmentOptimal,
            _ if usage.contains(Usage::SAMPLED) =>
                Layout::ShaderReadOnlyOptimal,
            _ if usage.contains(Usage::TRANSFER_SRC) =>
                Layout::TransferSrcOptimal,
            _ if usage.contains(Usage::TRANSFER_DST) =>
                Layout::TransferDstOptimal,
            _ => Layout::General,
        };
        let stable_state = (stable_access, stable_layout);
        let info = image::Info { aspects, usage, kind, mip_levels, format, origin, stable_state };
        let handle = handle::raw::Image::from(
            Image::new(image, info, self.garbage.clone()));
        let token = InitToken { handle: handle.clone().into() };
        Ok((handle, token))
    }

    pub fn create_image<F, A>(
        &mut self,
        allocator: &mut A,
        usage: image::Usage,
        kind: image::Kind,
        mip_levels: image::Level,
    ) -> Result<(handle::Image<B, F>, InitToken<B>), image::CreationError>
    where
        F: format::AsFormat,
        A: Allocator<B>,
    {
        self.create_image_raw(
            allocator,
            usage,
            kind,
            mip_levels,
            F::SELF,
        ).map(|(h, t)| (Typed::new(h), t))
    }

    pub fn create_image_view_raw(
        &mut self,
        image: &handle::raw::Image<B>,
        kind: image::ViewKind,
        format: format::Format,
        range: image::SubresourceRange,
    ) -> Result<handle::raw::ImageView<B>, image::ViewError> {
        self.raw.create_image_view(image.resource(), kind, format, format::Swizzle::NO, range)
            .map(|view| ImageView::new(
                view,
                image.clone(),
                self.garbage.clone()
            ).into())
    }

    pub fn create_image_view<F>(
        &mut self,
        image: &handle::Image<B, F>,
        kind: image::ViewKind,
        range: image::SubresourceRange,
    ) -> Result<handle::ImageView<B, F>, image::ViewError>
    where
        F: format::AsFormat,
    {
        self.create_image_view_raw(image.as_ref(), kind, F::SELF, range)
            .map(Typed::new)
    }

    pub fn create_sampler(&mut self, info: image::SamplerInfo) -> handle::Sampler<B> {
        handle::inner::Sampler::new(
            self.raw.create_sampler(info.clone()), info, self.garbage.clone()
        ).into()
    }

    // TODO: smarter allocation
    pub fn create_descriptors<D>(&mut self, count: usize) -> Vec<(D, D::Data)>
        where D: pso::Descriptors<B>
    {
        use hal::pso::DescriptorPool as CDP;

        let bindings = &D::layout_bindings()[..];
        let layout = self.create_descriptor_set_layout(bindings);
        let ranges = bindings.iter().map(|binding| {
            hal::pso::DescriptorRangeDesc {
                ty: binding.ty,
                count: binding.count * count,
            }
        }).collect::<Vec<_>>();

        let mut pool = self.raw.create_descriptor_pool(count, &ranges[..]);
        let sets = {
            let layout_refs = (0..count).map(|_| layout.resource());
            pool.allocate_sets(layout_refs)
        };

        let pool = handle::raw::DescriptorPool::from(
            DescriptorPool::new(pool, (), self.garbage.clone()));
        sets.into_iter().map(|set| {
            D::from_raw(layout.clone(), pso::RawDescriptorSet {
                resource: set,
                pool: pool.clone()
            })
        }).collect()
    }

    fn create_descriptor_set_layout(
        &mut self,
        bindings: &[hal::pso::DescriptorSetLayoutBinding],
    ) -> handle::raw::DescriptorSetLayout<B> {
        let layout = self.raw.create_descriptor_set_layout(bindings);
        DescriptorSetLayout::new(layout, (), self.garbage.clone()).into()
    }

    pub fn update_descriptor_sets(&mut self) -> pso::DescriptorSetsUpdate<B> {
        pso::DescriptorSetsUpdate::new(self)
    }

    #[doc(hidden)]
    pub fn create_render_pass_raw(
        &mut self,
        attachments: &[hal::pass::Attachment],
        subpasses: &[hal::pass::SubpassDesc],
        dependencies: &[hal::pass::SubpassDependency],
    ) -> handle::raw::RenderPass<B> {
        let pass = self.raw.create_render_pass(attachments, subpasses, dependencies);
        RenderPass::new(pass, (), self.garbage.clone()).into()
    }

    #[doc(hidden)]
    pub fn create_pipeline_layout_raw(
        &mut self,
        layouts: &[&B::DescriptorSetLayout],
        push_constant_ranges: &[(hal::pso::ShaderStageFlags, Range<u32>)],
    ) -> handle::raw::PipelineLayout<B> {
        let layout = self.raw.create_pipeline_layout(layouts.iter().cloned(), push_constant_ranges);
        PipelineLayout::new(layout, (), self.garbage.clone()).into()
    }

    #[doc(hidden)]
    pub fn create_graphics_pipeline_raw(
        &mut self,
        desc: hal::pso::GraphicsPipelineDesc<B>,
    ) -> Result<handle::raw::GraphicsPipeline<B>, pso::CreationError> {
        let pipeline = self.raw.create_graphics_pipelines(&[desc]).pop().unwrap()?;
        Ok(GraphicsPipeline::new(pipeline, (), self.garbage.clone()).into())
    }

    pub fn create_graphics_pipeline<I>(
        &mut self,
        shader_entries: hal::pso::GraphicsShaderSet<B>,
        primitive: Primitive,
        rasterizer: pso::Rasterizer,
        init: I,
    ) -> Result<I::Pipeline, pso::CreationError>
        where I: pso::GraphicsPipelineInit<B>
    {
        init.create(self, shader_entries, primitive, rasterizer)
    }

    // TODO?: typed
    pub fn create_framebuffer<P>(
        &mut self,
        pipeline: &P,
        attachments: &[&handle::raw::ImageView<B>],
        extent: image::Extent,
    ) -> Result<handle::raw::Framebuffer<B>, FramebufferError>
        where P: pso::GraphicsPipelineMeta<B>
    {
        let resources = attachments.iter().map(|&view| view.resource());
        let buffer = self.raw.create_framebuffer(pipeline.render_pass(), resources, extent)?;
        let info = handle::FramebufferInfo {
            attachments: attachments.iter().cloned().cloned().collect(),
            extent,
        };
        Ok(Framebuffer::new(buffer, info, self.garbage.clone()).into())
    }
/*
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

    /// Create a linear sampler with clamping to border.
    fn create_sampler_linear(&mut self) -> handle::Sampler<B> {
        self.create_sampler(texture::SamplerInfo::new(
            texture::FilterMethod::Trilinear,
            texture::WrapMode::Clamp,
        ))
    }
    */
}

fn range_in_bytes<T>(elements: Range<u64>) -> Range<u64> {
    let stride = mem::size_of::<T>() as u64;
    (elements.start * stride)..(elements.end * stride)
}
