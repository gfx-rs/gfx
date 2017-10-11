use std::mem;
use std::ops::Range;
use core::{Device as CoreDevice, MemoryType};
use core::memory::{Properties,
    DEVICE_LOCAL, CPU_VISIBLE, CPU_CACHED, WRITE_COMBINED
};

use memory::{self, Allocator, Typed};
use handle::{self, GarbageSender};
use handle::inner::*;
use {core, buffer, image, format, mapping, pso};
use {Backend, Primitive, Extent};

pub use core::device::{TargetViewError, FrameBufferError};

#[derive(Clone)]
pub struct Device<B: Backend> {
    raw: B::Device,
    // TODO: could be shared instead of cloned
    memory_types: Vec<MemoryType>,
    memory_heaps: Vec<u64>,
    garbage: GarbageSender<B>,
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
        let (garbage, collector) = handle::garbage(&raw);
        (Device { raw, memory_types, memory_heaps, garbage }, collector)
    }

    pub fn memory_types(&self) -> &[MemoryType] {
        &self.memory_types
    }

    pub fn memory_heaps(&self) -> &[u64] {
        &self.memory_heaps
    }

    pub fn ref_raw(&self) -> &B::Device {
        &self.raw
    }

    pub fn mut_raw(&mut self) -> &mut B::Device {
        &mut self.raw
    }

    pub fn find_memory<P>(&self, type_mask: u64, predicate: P) -> Option<MemoryType>
        where P: Fn(Properties) -> bool
    {
        self.memory_types.iter()
            .find(|memory_type| {
                type_mask & (1 << memory_type.id) != 0 &&
                predicate(memory_type.properties)
            })
            .cloned()
    }

    pub fn find_usage_memory(&self, usage: memory::Usage, type_mask: u64) -> Option<MemoryType> {
        use memory::Usage::*;
        match usage {
            Data => self.find_data_memory(type_mask),
            Upload => self.find_upload_memory(type_mask),
            Download => self.find_download_memory(type_mask),
        }
    }

    // TODO: fallbacks when out of memory

    pub fn find_data_memory(&self, type_mask: u64) -> Option<MemoryType> {
        self.find_memory(type_mask, |props| {
            props.contains(DEVICE_LOCAL) && !props.contains(CPU_VISIBLE)
        }).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(DEVICE_LOCAL)
        }))
    }

    pub fn find_upload_memory(&self, type_mask: u64) -> Option<MemoryType> {
        self.find_memory(type_mask, |props| {
            props.contains(CPU_VISIBLE | WRITE_COMBINED)
            && !props.contains(CPU_CACHED)
        }).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(CPU_VISIBLE)
        }))
    }

    pub fn find_download_memory(&self, type_mask: u64) -> Option<MemoryType> {
        self.find_memory(type_mask, |props| {
            props.contains(CPU_VISIBLE | CPU_CACHED)
            && !props.contains(WRITE_COMBINED)
        }).or_else(|| self.find_memory(type_mask, |props| {
            props.contains(CPU_VISIBLE)
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
        let buffer = self.raw.create_buffer(size, stride, usage)?;
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

    /// Acquire a mapping Reader.
    ///
    /// The accessible slice will correspond to the specified range (in elements).
    /// See `acquire_mapping_writer` for more information.
    pub fn acquire_mapping_reader<'a, MTB>(
        &mut self,
        buffer: &'a MTB,
        range: Range<u64>,
    ) -> Result<mapping::Reader<'a, B, MTB::Data>, mapping::Error>
        where MTB: buffer::MaybeTyped<B>
    {
        let (resource, info) = buffer.as_raw().resource_info();
        assert!(info.access.acquire_exclusive(), "access overlap on mapping");
        Ok(mapping::Reader {
            inner: self.raw.acquire_mapping_reader(
                resource,
                range_in_bytes::<MTB::Data>(range)
            )?,
            info,
        })
    }

    /// Release a mapping Reader.
    ///
    /// See `acquire_mapping_writer` for more information.
    pub fn release_mapping_reader<'a, T>(
        &mut self,
        reader: mapping::Reader<'a, B, T>
    ) {
        self.raw.release_mapping_reader(reader.inner);
        reader.info.access.release_exclusive();
    }

    /// Acquire a mapping Writer.
    ///
    /// The accessible slice will correspond to the specified range (in elements).
    ///
    /// While holding this access, you hold CPU-side exclusive access.
    /// Any access overlap will panic.
    /// Submitting commands involving this buffer to the device
    /// implicitly requires exclusive access until frame synchronisation
    /// on `acquire_frame`.
    pub fn acquire_mapping_writer<'a, MTB>(
        &mut self,
        buffer: &'a MTB,
        range: Range<u64>,
    ) -> Result<mapping::Writer<'a, B, MTB::Data>, mapping::Error>
        where MTB: buffer::MaybeTyped<B>
    {
        let (resource, info) = buffer.as_raw().resource_info();
        assert!(info.access.acquire_exclusive(), "access overlap on mapping");
        Ok(mapping::Writer {
            inner: self.raw.acquire_mapping_writer(
                resource,
                range_in_bytes::<MTB::Data>(range)
            )?,
            info,
        })
    }

    /// Release a mapping Writer.
    ///
    /// See `acquire_mapping_writer` for more information.
    pub fn release_mapping_writer<'a, T>(
        &mut self,
        writer: mapping::Writer<'a, B, T>
    ) {
        self.raw.release_mapping_writer(writer.inner);
        writer.info.access.release_exclusive();
    }

    /// Sugar to acquire and release a mapping reader.
    pub fn read_mapping<'a, MTB>(
        &'a mut self,
        buffer: &'a MTB,
        range: Range<u64>
    ) -> Result<mapping::ReadScope<'a, B, MTB::Data>, mapping::Error>
        where MTB: buffer::MaybeTyped<B>
    {
        let reader = self.acquire_mapping_reader(buffer, range)?;
        Ok(mapping::ReadScope {
            reader: Some(reader),
            device: self,
        })
    }

    /// Sugar to acquire and release a mapping writer.
    pub fn write_mapping<'a, MTB>(
        &'a mut self,
        buffer: &'a MTB,
        range: Range<u64>
    ) -> Result<mapping::WriteScope<'a, B, MTB::Data>, mapping::Error>
        where MTB: buffer::MaybeTyped<B>
    {
        let writer = self.acquire_mapping_writer(buffer, range)?;
        Ok(mapping::WriteScope {
            writer: Some(writer),
            device: self,
        })
    }

    pub fn create_image_raw<A>(
        &mut self,
        allocator: &mut A,
        usage: image::Usage,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format
    ) -> Result<(handle::raw::Image<B>, InitToken<B>), image::CreationError>
        where A: Allocator<B>
    {
        use image::{
            COLOR_ATTACHMENT, DEPTH_STENCIL_ATTACHMENT,
            SAMPLED, TRANSFER_SRC, TRANSFER_DST,
        };
        use core::image::ImageLayout;

        let image = self.raw.create_image(kind, mip_levels, format, usage)?;
        let (image, memory) = allocator.allocate_image(self, usage, image);
        let origin = image::Origin::User(memory);
        let stable_access = core::image::Access::empty();
        let stable_layout = match usage {
            _ if usage.contains(COLOR_ATTACHMENT) =>
                ImageLayout::ColorAttachmentOptimal,
            _ if usage.contains(DEPTH_STENCIL_ATTACHMENT) =>
                ImageLayout::DepthStencilAttachmentOptimal,
            _ if usage.contains(SAMPLED) =>
                ImageLayout::ShaderReadOnlyOptimal,
            _ if usage.contains(TRANSFER_SRC) =>
                ImageLayout::TransferSrcOptimal,
            _ if usage.contains(TRANSFER_DST) =>
                ImageLayout::TransferDstOptimal,
            _ => ImageLayout::General,
        };
        let stable_state = (stable_access, stable_layout);
        let info = image::Info { usage, kind, mip_levels, format, origin, stable_state };
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
        where F: format::Formatted,
              A: Allocator<B>
    {
        self.create_image_raw(
            allocator,
            usage,
            kind,
            mip_levels,
            F::get_format()
        ).map(|(h, t)| (Typed::new(h), t))
    }

    pub fn create_sampler(&mut self, info: image::SamplerInfo)
        -> handle::Sampler<B>
    {
        handle::inner::Sampler::new(
            self.raw.create_sampler(info.clone()), info, self.garbage.clone()
        ).into()
    }

    pub fn view_buffer_as_constant_raw(
        &mut self,
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

    pub fn view_buffer_as_constant<T>(
        &mut self,
        buffer: &handle::Buffer<B, T>,
        range: Range<u64>,
    ) -> Result<handle::ConstantBufferView<B, T>, TargetViewError>
    {
        self.view_buffer_as_constant_raw(buffer, range)
            .map(Typed::new)
    }

    pub fn view_image_as_render_target_raw(
        &mut self,
        image: &handle::raw::Image<B>,
        format: format::Format,
        layers: image::SubresourceLayers
    ) -> Result<handle::raw::RenderTargetView<B>, TargetViewError> {
        self.raw.view_image_as_render_target(image.resource(), format, layers)
            .map(|rtv| RenderTargetView::new(
                rtv,
                image.clone(),
                self.garbage.clone()
            ).into())
    }

    pub fn view_image_as_render_target<F>(
        &mut self,
        image: &handle::Image<B, F>,
        range: image::SubresourceLayers
    ) -> Result<handle::RenderTargetView<B, F>, TargetViewError>
        where F: format::RenderFormat
    {
        self.view_image_as_render_target_raw(image, F::get_format(), range)
            .map(Typed::new)
    }

    pub fn view_image_as_depth_stencil_raw(
        &mut self,
        image: &handle::raw::Image<B>,
        format: format::Format,
        layers: image::SubresourceLayers,
    ) -> Result<handle::raw::RenderTargetView<B>, TargetViewError>
    {
        self.raw.view_image_as_render_target(image.resource(), format, layers)
            .map(|rtv| RenderTargetView::new(
                rtv,
                image.clone(),
                self.garbage.clone()
            ).into())
    }

    pub fn view_image_as_depth_stencil<F>(
        &mut self,
        image: &handle::Image<B, F>,
        layers: image::SubresourceLayers,
    ) -> Result<handle::RenderTargetView<B, F>, TargetViewError>
        where F: format::RenderFormat
    {
        self.view_image_as_render_target_raw(image, F::get_format(), layers)
            .map(Typed::new)
    }

    pub fn view_image_as_shader_resource_raw(
        &mut self,
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

    pub fn view_image_as_unordered_access_raw(
        &mut self,
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

    // TODO: smarter allocation
    pub fn create_descriptors<D>(&mut self, count: usize) -> Vec<(D, D::Data)>
        where D: pso::Descriptors<B>
    {
        use core::DescriptorPool as CDP;

        let bindings = &D::layout_bindings()[..];
        let layout = self.create_descriptor_set_layout(bindings);
        let ranges = bindings.iter().map(|binding| {
            core::pso::DescriptorRangeDesc {
                ty: binding.ty,
                count: binding.count * count,
            }
        }).collect::<Vec<_>>();

        let mut pool = self.raw.create_descriptor_pool(count, &ranges[..]);
        let sets = {
            let layout_refs = (0..count).map(|_| layout.resource())
                .collect::<Vec<_>>();
            pool.allocate_sets(&layout_refs[..])
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
        bindings: &[core::pso::DescriptorSetLayoutBinding]
    ) -> handle::raw::DescriptorSetLayout<B> {
        let layout = self.raw.create_descriptor_set_layout(bindings);
        DescriptorSetLayout::new(layout, (), self.garbage.clone()).into()
    }

    pub fn update_descriptor_sets(&mut self) -> pso::DescriptorSetsUpdate<B> {
        pso::DescriptorSetsUpdate::new(self)
    }

    #[doc(hidden)]
    pub fn create_renderpass_raw(
        &mut self,
        attachments: &[core::pass::Attachment],
        subpasses: &[core::pass::SubpassDesc],
        dependencies: &[core::pass::SubpassDependency],
    ) -> handle::raw::RenderPass<B> {
        let pass = self.raw.create_renderpass(attachments, subpasses, dependencies);
        RenderPass::new(pass, (), self.garbage.clone()).into()
    }

    #[doc(hidden)]
    pub fn create_pipeline_layout_raw(
        &mut self,
        layouts: &[&B::DescriptorSetLayout]
    ) -> handle::raw::PipelineLayout<B> {
        let layout = self.raw.create_pipeline_layout(layouts);
        PipelineLayout::new(layout, (), self.garbage.clone()).into()
    }

    #[doc(hidden)]
    pub fn create_graphics_pipeline_raw(
        &mut self,
        shader_entries: core::pso::GraphicsShaderSet<B>,
        layout: &B::PipelineLayout,
        subpass: core::pass::Subpass<B>,
        desc: &core::pso::GraphicsPipelineDesc,
    ) -> Result<handle::raw::GraphicsPipeline<B>, pso::CreationError> {
        let pipeline = self.raw.create_graphics_pipelines(&[
            (shader_entries, layout, subpass, desc)
        ]).pop().unwrap()?;
        Ok(GraphicsPipeline::new(pipeline, (), self.garbage.clone()).into())
    }

    pub fn create_graphics_pipeline<I>(
        &mut self,
        shader_entries: core::pso::GraphicsShaderSet<B>,
        primitive: Primitive,
        rasterizer: pso::Rasterizer,
        init: I
    ) -> Result<I::Pipeline, pso::CreationError>
        where I: pso::GraphicsPipelineInit<B>
    {
        init.create(self, shader_entries, primitive, rasterizer)
    }

    // TODO?: typed
    pub fn create_framebuffer<P>(
        &mut self,
        pipeline: &P,
        rtvs: &[&handle::raw::RenderTargetView<B>],
        dsvs: &[&handle::raw::DepthStencilView<B>],
        extent: Extent,
    ) -> Result<handle::raw::FrameBuffer<B>, FrameBufferError>
        where P: pso::GraphicsPipelineMeta<B>
    {
        let rtv_res: Vec<_> = rtvs.iter().map(|&rtv| rtv.resource()).collect();
        let dsv_res: Vec<_> = dsvs.iter().map(|&dsv| dsv.resource()).collect();
        let buffer = self.raw.create_framebuffer(
            pipeline.render_pass(), &rtv_res[..], &dsv_res[..], extent)?;
        let info = handle::FrameBufferInfo {
            rtvs: rtvs.iter().map(|&rtv| rtv.clone()).collect(),
            dsvs: dsvs.iter().map(|&dsv| dsv.clone()).collect(),
            extent,
        };
        Ok(FrameBuffer::new(buffer, info, self.garbage.clone()).into())
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
