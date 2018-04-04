//! A typed high-level pipeline interface.

use std::borrow::Borrow;
use std::mem;
use std::marker::PhantomData;

use {hal, format, handle};
use hal::image::Layout;
use hal::pass::{AttachmentOps, AttachmentLoadOp, AttachmentStoreOp};

use {Backend, Device, Primitive, Supports, Transfer, Graphics, Encoder};

pub use hal::pso::{
    ColorValue, DepthValue, StencilValue, Rect, Viewport,
    DescriptorBinding, DescriptorArrayIndex, Rasterizer, CreationError, InstanceRate,
};

#[derive(Debug)]
pub struct RawDescriptorSet<B: Backend> {
    pub(crate) resource: B::DescriptorSet,
    pub(crate) pool: handle::raw::DescriptorPool<B>,
}

impl<B: Backend> RawDescriptorSet<B> {
    pub fn resource(&self) -> &B::DescriptorSet { &self.resource }
}

pub trait Descriptors<B: Backend>: Sized {
    type Data: Sized;

    fn from_raw(handle::raw::DescriptorSetLayout<B>, RawDescriptorSet<B>) -> (Self, Self::Data);
    fn layout_bindings() -> Vec<hal::pso::DescriptorSetLayoutBinding>;
    fn layout(&self) -> &B::DescriptorSetLayout;
    fn set(&self) -> &B::DescriptorSet;
}

pub trait BindDesc {
    const TYPE: hal::pso::DescriptorType;
    const COUNT: usize;
}

pub trait Bind<B: Backend>: BindDesc {
    type Handle: 'static + Clone;

    fn write<'a, I>(views: I) -> Vec<hal::pso::Descriptor<'a, B>>
    where
        I: IntoIterator,
        I::Item: Borrow<&'a Self::Handle>;

    fn require<'a>(
        &'a Self::Handle,
        &mut Vec<(&'a handle::raw::Buffer<B>, hal::buffer::State)>,
        &mut Vec<(&'a handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
        &mut handle::Bag<B>,
    );
}

macro_rules! define_descriptors {
    ([$( $array_len:expr ),*] $( $name:ident, )*) => {
        $(
            impl<T: BindDesc> BindDesc for [T; $array_len] {
                const TYPE: hal::pso::DescriptorType = T::TYPE;
                const COUNT: usize = $array_len * T::COUNT;
            }

            impl<B, T> Bind<B> for [T; $array_len]
                where B: Backend, T: Bind<B>
            {
                type Handle = T::Handle;

                fn write<'a, I>(handles: I) -> Vec<hal::pso::Descriptor<'a, B>>
                where
                    I: IntoIterator,
                    I::Item: Borrow<&'a Self::Handle>
                {
                    T::write(handles)
                }

                fn require<'a>(
                    handle: &'a Self::Handle,
                    buffers: &mut Vec<(&'a handle::raw::Buffer<B>, hal::buffer::State)>,
                    images: &mut Vec<(&'a handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
                    others: &mut handle::Bag<B>
                ) {
                    T::require(handle, buffers, images, others)
                }
            }
        )*
        $(
            pub struct $name;

            impl BindDesc for $name {
                const TYPE: hal::pso::DescriptorType = hal::pso::DescriptorType::$name;
                const COUNT: usize = 1;
            }
        )*
    }
}

// TODO: type-safe formats
define_descriptors! {
    [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12 ]
    SampledImage,
    Sampler,
}

impl<B: Backend> Bind<B> for SampledImage {
    type Handle = handle::raw::ImageView<B>;

    fn write<'a, I>(_views: I) -> Vec<hal::pso::Descriptor<'a, B>>
    where
        I: IntoIterator,
        I::Item: Borrow<&'a Self::Handle>,
    {
        Vec::new()
        /* views
            .into_iter()
            .map(|view| {
                let layout = Layout::ShaderReadOnlyOptimal;
                (view.borrow().resource(), layout)
            }).collect())*/
    }

    fn require<'a>(
        view: &'a Self::Handle,
        _: &mut Vec<(&'a handle::raw::Buffer<B>, hal::buffer::State)>,
        images: &mut Vec<(&'a handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
        _: &mut handle::Bag<B>,
    ) {
        let img = view.info();
        let levels = img.info().mip_levels;
        let layers = img.info().kind.num_layers();
        let state = (hal::image::Access::SHADER_READ, Layout::ShaderReadOnlyOptimal);
        for level in 0..levels {
            for layer in 0..layers {
                let subresource = hal::image::Subresource {
                    aspects: img.info().aspects,
                    level,
                    layer
                };
                images.push((img, subresource, state));
            }
        }
    }
}

impl<B: Backend> Bind<B> for Sampler {
    type Handle = handle::raw::Sampler<B>;

    fn write<'a, I>(_samplers: I) -> Vec<hal::pso::Descriptor<'a, B>>
    where
        I: IntoIterator,
        I::Item: Borrow<&'a Self::Handle>,
    {
        Vec::new()
        /*
        samplers
            .into_iter()
            .map(|sampler| sampler.borrow().resource())
            .collect())*/
    }

    fn require<'a>(
        sampler: &'a Self::Handle,
        _: &mut Vec<(&'a handle::raw::Buffer<B>, hal::buffer::State)>,
        _: &mut Vec<(&'a handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
        others: &mut handle::Bag<B>,
    ) {
        others.add(sampler.clone());
    }
}

pub struct DescriptorSetBindRef<'a, 'b, B: Backend, T: Bind<B>> {
    pub set: &'a B::DescriptorSet,
    pub binding: DescriptorBinding,
    pub handles: &'b mut [Option<T::Handle>],
}

pub struct DescriptorSetsUpdate<'a, B: Backend> {
    device: &'a mut Device<B>,
    writes: Vec<hal::pso::DescriptorSetWrite<'a, B, Vec<hal::pso::Descriptor<'a, B>>>>,
}

impl<'a, B: Backend> DescriptorSetsUpdate<'a, B> {
    pub(crate) fn new(device: &'a mut Device<B>) -> Self {
        DescriptorSetsUpdate { device, writes: Vec::new() }
    }

    pub fn write<'b, T: Bind<B>, I>(
        mut self,
        bind_ref: DescriptorSetBindRef<'a, 'b, B, T>,
        array_offset: usize,
        handles: I,
    ) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<&'a T::Handle>,
    {
        let handles: Vec<_> = handles.into_iter().map(|handle| *handle.borrow()).collect();
        for (slot, &handle) in bind_ref.handles[array_offset..].iter_mut().zip(handles.iter()) {
            *slot = Some(handle.clone());
        }

        self.writes.push(hal::pso::DescriptorSetWrite {
            set: bind_ref.set,
            binding: bind_ref.binding,
            array_offset: 0,
            descriptors: T::write(handles),
        });
        self
    }

    pub fn finish(self) {
        use hal::Device;
        self.device.raw.write_descriptor_sets(self.writes);
    }
}

pub trait GraphicsPipelineInit<B: Backend> {
    type Pipeline;

    fn create<'a>(
        self,
        &mut Device<B>,
        hal::pso::GraphicsShaderSet<'a, B>,
        Primitive,
        Rasterizer
    ) -> Result<Self::Pipeline, CreationError>;
}

pub trait GraphicsPipelineMeta<B: Backend> {
    fn layout(&self) -> &B::PipelineLayout;
    fn render_pass(&self) -> &B::RenderPass;
}

pub trait GraphicsPipelineData<B: Backend> {
    type Pipeline;

    fn begin_renderpass<'a, 'b, C>(
        self,
        encoder: &'a mut Encoder<'b, B, C>,
        pipeline: &'a Self::Pipeline
    ) -> hal::command::RenderPassInlineEncoder<'a, B, hal::command::Primary>
        where Self: 'a, 'b: 'a, C: Supports<Transfer> + Supports<Graphics>;
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
        &mut hal::pso::GraphicsPipelineDesc<B>,
    ) {}

    fn require<'b>(
        &'b Self::Data,
        &mut Vec<(&'b handle::raw::Buffer<B>, hal::buffer::State)>,
        &mut Vec<(&'b handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
        &mut handle::Bag<B>,
    ) where 'a: 'b {}

    fn vertex_buffer<'b>(&'b Self::Data) -> Option<(&'b B::Buffer, hal::buffer::Offset)>
        where 'a: 'b
    {
        None
    }

    fn descriptor_set<'b>(&'b Self::Data) -> Option<&'b B::DescriptorSet>
        where 'a: 'b
    {
        None
    }
}

pub struct Attachment {
    pub format: format::Format,
    pub ops: AttachmentOps,
    pub stencil_ops: AttachmentOps,
    pub required_layout: Layout,
}

pub struct RenderTarget<F: format::AsFormat>(PhantomData<F>);

impl<'a, B, F> Component<'a, B> for RenderTarget<F>
where
    B: Backend,
    F: 'a + format::AsFormat,
{
    type Init = hal::pso::ColorBlendDesc;
    type Data = &'a handle::ImageView<B, F>;

    fn attachment(_: &Self::Init) -> Option<Attachment> {
        Some(Attachment {
            format: F::SELF,
            // TODO: AttachmentLoadOp::Clear
            ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            required_layout: Layout::ColorAttachmentOptimal,
        })
    }

    fn append_desc(
        init: Self::Init,
        pipeline_desc: &mut hal::pso::GraphicsPipelineDesc<B>,
    ) {
        pipeline_desc.blender.targets.push(init);
    }

    fn require<'b>(
        data: &'b Self::Data,
        _: &mut Vec<(&'b handle::raw::Buffer<B>, hal::buffer::State)>,
        images: &mut Vec<(&'b handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
        _: &mut handle::Bag<B>,
    ) where 'a: 'b {
        let img = data.as_ref().info();
        let levels = img.info().mip_levels;
        let layers = img.info().kind.num_layers();
        // TODO: READ not always necessary
        let state = (hal::image::Access::COLOR_ATTACHMENT_READ | hal::image::Access::COLOR_ATTACHMENT_WRITE,
            Layout::ColorAttachmentOptimal);
        for level in 0..levels {
            for layer in 0..layers {
                let subresource = hal::image::Subresource {
                    aspects: img.info().aspects,
                    level,
                    layer
                };
                images.push((img, subresource, state));
            }
        }
    }
}

pub trait Structure: Sized {
    fn elements() -> Vec<hal::pso::Element<format::Format>>;
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
        pipeline_desc: &mut hal::pso::GraphicsPipelineDesc<B>,
    ) {
        let binding = pipeline_desc.vertex_buffers.len() as u32;
        pipeline_desc.vertex_buffers.push(hal::pso::VertexBufferDesc {
            stride: mem::size_of::<T>() as u32,
            rate: I::get_rate(&init),
        });
        let mut location = 0;
        for element in T::elements() {
            pipeline_desc.attributes.push(hal::pso::AttributeDesc {
                location,
                binding,
                element,
            });
            location += 1;
        }
    }

    fn require<'b>(
        data: &'b Self::Data,
        buffers: &mut Vec<(&'b handle::raw::Buffer<B>, hal::buffer::State)>,
        _: &mut Vec<(&'b handle::raw::Image<B>, hal::image::Subresource, hal::image::State)>,
        _: &mut handle::Bag<B>,
    ) where 'a: 'b {
        buffers.push((data.as_ref(), hal::buffer::Access::VERTEX_BUFFER_READ));
    }

    fn vertex_buffer<'b>(data: &'b Self::Data) -> Option<(&'b B::Buffer, hal::buffer::Offset)>
        where 'a: 'b
    {
        // TODO: offset
        Some((data.as_ref().resource(), 0))
    }
}

pub type InstanceBuffer<T> = VertexBuffer<T, Instanced>;
