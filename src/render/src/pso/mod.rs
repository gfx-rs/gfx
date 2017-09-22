//! A typed high-level pipeline interface.

use std::mem;
use std::marker::PhantomData;

use {core, handle};
use core::image::ImageLayout;
use core::pass::{AttachmentOps, AttachmentLoadOp, AttachmentStoreOp};
use format::{self, Format};
use {Backend, Device, Primitive};

pub use core::pso::{Rasterizer, CreationError, InstanceRate};

#[derive(Debug)]
pub struct RawDescriptorSet<B: Backend> {
    pub(crate) resource: B::DescriptorSet,
    pub(crate) pool: handle::raw::DescriptorPool<B>,
}

impl<B: Backend> RawDescriptorSet<B> {
    pub fn resource(&self) -> &B::DescriptorSet { &self.resource }
}

pub trait Descriptors<B: Backend> {
    fn from_raw(handle::raw::DescriptorSetLayout<B>, RawDescriptorSet<B>) -> Self;
    fn layout_bindings() -> Vec<core::pso::DescriptorSetLayoutBinding>;
    fn layout(&self) -> &B::DescriptorSetLayout;
    fn set(&self) -> &B::DescriptorSet;
}

pub trait Bind {
    fn desc_type() -> core::pso::DescriptorType;
    fn desc_count() -> usize;
}

pub trait BindWrite<'a, B: Backend> {
    type Input: 'a;
    fn write(input: Self::Input) -> core::pso::DescriptorWrite<'a, B>;
}

macro_rules! define_descriptors {
    ([$( $array_len:expr ),*] $( $name:ident, )*) => {
        $(
            impl<T: Bind> Bind for [T; $array_len] {
                fn desc_type() -> core::pso::DescriptorType {
                    T::desc_type()
                }
                fn desc_count() -> usize { $array_len * T::desc_count() }
            }
            impl<'a, B, T> BindWrite<'a, B> for [T; $array_len]
                where B: Backend, T: BindWrite<'a, B>
            {
                type Input = T::Input;
                fn write(input: Self::Input) -> core::pso::DescriptorWrite<'a, B> {
                    T::write(input)
                }
            }
        )*
        $(
            pub struct $name;
            impl Bind for $name {
                fn desc_type() -> core::pso::DescriptorType {
                    core::pso::DescriptorType::$name
                }
                fn desc_count() -> usize { 1 }
            }
        )*
    }
}

define_descriptors! {
    [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12 ]
    SampledImage,
    Sampler,
}

impl<'a, B: Backend> BindWrite<'a, B> for SampledImage {
    type Input = Vec<&'a handle::raw::ShaderResourceView<B>>;
    fn write(srvs: Self::Input) -> core::pso::DescriptorWrite<'a, B> {
        core::pso::DescriptorWrite::SampledImage(srvs.into_iter()
            .map(|srv| match srv.info() {
                &handle::ViewSource::Image(_) => {
                    let layout = ImageLayout::ShaderReadOnlyOptimal;
                    (srv.resource(), layout)
                }
                &handle::ViewSource::Buffer(_) => unreachable!(),
            }).collect())
    }
}

impl<'a, B: Backend> BindWrite<'a, B> for Sampler {
    type Input = Vec<&'a handle::raw::Sampler<B>>;
    fn write(samplers: Self::Input) -> core::pso::DescriptorWrite<'a, B> {
        core::pso::DescriptorWrite::Sampler(samplers.into_iter()
            .map(|sampler| sampler.resource())
            .collect())
    }
}

pub struct DescriptorSetBindRef<'a, B: Backend, T> {
    pub set: &'a B::DescriptorSet,
    pub binding: usize,
    pub phantom: PhantomData<T>,
}

pub struct DescriptorSetsUpdate<'a, B: Backend> {
    device: &'a mut Device<B>,
    writes: Vec<core::pso::DescriptorSetWrite<'a, 'a, B>>,
}

impl<'a, B: Backend> DescriptorSetsUpdate<'a, B> {
    pub(crate) fn new(device: &'a mut Device<B>) -> Self {
        DescriptorSetsUpdate { device, writes: Vec::new() }
    }

    pub fn write<T: BindWrite<'a, B>>(
        mut self,
        bind_ref: DescriptorSetBindRef<'a, B, T>,
        array_offset: usize,
        write: T::Input
    ) -> Self {
        self.writes.push(core::pso::DescriptorSetWrite {
            set: bind_ref.set,
            binding: bind_ref.binding,
            array_offset,
            write: T::write(write)
        });
        self
    }

    pub fn finish(self) {
        use core::Device;
        self.device.mut_raw().update_descriptor_sets(&self.writes[..]);
    }
}

pub trait GraphicsPipelineInit<B: Backend> {
    type Pipeline;
    fn create(
        self,
        &mut Device<B>,
        core::pso::GraphicsShaderSet<B>,
        Primitive,
        Rasterizer
    ) -> Result<Self::Pipeline, CreationError>;
}

pub trait GraphicsPipelineMeta<B: Backend> {
    fn layout(&self) -> &B::PipelineLayout;
    fn render_pass(&self) -> &B::RenderPass;
    fn pipeline(&self) -> &B::GraphicsPipeline;
}

pub trait GraphicsPipelineData<B: Backend> {
    type Pipeline;
    fn bind(
        self,
        viewport: core::Viewport,
        scissor: core::target::Rect,
        pipeline: &Self::Pipeline
    ); // TODO
}

pub trait Component<'a, B: Backend> {
    type Init: 'a;
    type Data: 'a;

    fn descriptor_layout<'b>(&'b Self::Init) -> Option<&'b B::DescriptorSetLayout>
        where 'a: 'b
    {
        None
    }
    fn attachment(&Self::Init) -> Option<Attachment> {
        None
    }
    fn append_desc(
        Self::Init,
        &mut core::pso::GraphicsPipelineDesc,
    ) {}
}

pub struct DescriptorSet<D>(PhantomData<D>);
impl<'a, B: Backend, D: 'a + Descriptors<B>> Component<'a, B> for DescriptorSet<D> {
    type Init = &'a D;
    type Data = &'a D;

    fn descriptor_layout<'b>(init: &'b Self::Init) -> Option<&'b B::DescriptorSetLayout>
        where 'a: 'b
    {
        Some(init.layout())
    }
}

pub struct Attachment {
    pub format: Format,
    pub ops: AttachmentOps,
    pub stencil_ops: AttachmentOps,
    pub required_layout: ImageLayout,
}

pub struct RenderTarget<F: format::RenderFormat>(PhantomData<F>);
impl<'a, B, F> Component<'a, B> for RenderTarget<F>
    where B: Backend, F: 'a + format::RenderFormat
{
    type Init = core::pso::ColorInfo;
    type Data = &'a handle::RenderTargetView<B, F>;

    fn attachment(_: &Self::Init) -> Option<Attachment> {
        Some(Attachment {
            format: F::get_format(),
            // TODO: AttachmentLoadOp::Clear
            ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            required_layout: ImageLayout::ColorAttachmentOptimal,
        })
    }

    fn append_desc(
        init: Self::Init,
        pipeline_desc: &mut core::pso::GraphicsPipelineDesc
    ) {
        pipeline_desc.blender.targets.push(init);
    }
}

pub trait Structure: Sized {
    fn elements() -> Vec<core::pso::Element<Format>>;
}

/// Helper trait to support variable instance rate.
pub trait ToInstanceRate {
    /// The associated init type for PSO component.
    type Init;
    /// Get an actual instance rate value from the init.
    fn get_rate(init: &Self::Init) -> InstanceRate;
}

/// Helper phantom type for per-vertex attributes.
pub enum NonInstanced {}
/// Helper phantom type for per-instance attributes.
pub enum Instanced {}

impl ToInstanceRate for InstanceRate {
    type Init = InstanceRate;
    fn get_rate(init: &Self::Init) -> InstanceRate { *init }
}
impl ToInstanceRate for Instanced {
    type Init = ();
    fn get_rate(_: &Self::Init) -> InstanceRate { 1 }
}
impl ToInstanceRate for NonInstanced {
    type Init = ();
    fn get_rate(_: &Self::Init) -> InstanceRate { 0 }
}

pub struct VertexBuffer<T: Structure, I=NonInstanced>(PhantomData<(T, I)>);
impl<'a, B, T, I> Component<'a, B> for VertexBuffer<T, I>
    where B: Backend, T: 'a + Structure, I: ToInstanceRate, I::Init: 'a
{
    type Init = I::Init;
    type Data = &'a handle::Buffer<B, T>;

    fn append_desc(
        init: Self::Init,
        pipeline_desc: &mut core::pso::GraphicsPipelineDesc
    ) {
        let binding = pipeline_desc.vertex_buffers.len() as u32;
        pipeline_desc.vertex_buffers.push(core::pso::VertexBufferDesc {
            stride: mem::size_of::<T>() as u32,
            rate: I::get_rate(&init),
        });
        let mut location = 0;
        for element in T::elements() {
            pipeline_desc.attributes.push(core::pso::AttributeDesc {
                location,
                binding,
                element,
            });
            location += 1;
        }
    }
}

pub type InstanceBuffer<T> = VertexBuffer<T, Instanced>;
