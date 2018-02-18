use std::iter::Extend;
use std::sync::{mpsc, Arc};

use memory::{Typed, Provider, Dependency};
use Backend;

pub(crate) fn garbage<B: Backend>(device: &Arc<B::Device>)
    -> (GarbageSender<B>, GarbageCollector<B>)
{
    let (sender, receiver) = mpsc::channel();
    let provider = Provider::new(InnerGarbageCollector {
        device: Arc::clone(device),
        receiver,
    });
    let dependency = provider.dependency();
    (GarbageSender { sender, dependency }, GarbageCollector(provider))
}

#[derive(Clone, Debug)]
pub(crate) struct GarbageSender<B: Backend> {
    sender: mpsc::Sender<Garbage<B>>,
    dependency: Dependency<InnerGarbageCollector<B>>,
}

pub(crate) struct GarbageCollector<B: Backend>(Provider<InnerGarbageCollector<B>>);

struct InnerGarbageCollector<B: Backend> {
    device: Arc<B::Device>,
    receiver: mpsc::Receiver<Garbage<B>>,
}

impl<B: Backend> GarbageSender<B> {
    pub fn send(&self, garbage: Garbage<B>)
        -> Result<(), mpsc::SendError<Garbage<B>>>
    {
        self.sender.send(garbage)
    }
}

impl<B: Backend> GarbageCollector<B> {
    pub fn collect(&mut self) {
        self.0.collect();
    }
}

impl<B: Backend> InnerGarbageCollector<B> {
    fn collect(&mut self) {
        use hal::Device;

        let dev = &mut self.device;
        for garbage in self.receiver.try_iter() {
            use self::Garbage::*;
            match garbage {
                // ShaderLib(sl) => dev.destroy_shader_lib(sl),
                RenderPass(rp) => dev.destroy_render_pass(rp),
                PipelineLayout(pl) => dev.destroy_pipeline_layout(pl),
                GraphicsPipeline(pl) => dev.destroy_graphics_pipeline(pl),
                Framebuffer(fb) => dev.destroy_framebuffer(fb),
                Buffer(b) => dev.destroy_buffer(b),
                BufferView(bv) => dev.destroy_buffer_view(bv),
                Image(i) => dev.destroy_image(i),
                ImageView(iv) => dev.destroy_image_view(iv),
                Sampler(s) => dev.destroy_sampler(s),
                DescriptorPool(dp) => dev.destroy_descriptor_pool(dp),
                DescriptorSetLayout(dsl) => dev.destroy_descriptor_set_layout(dsl),
            }
        }
    }
}

impl<B: Backend> Drop for InnerGarbageCollector<B> {
    fn drop(&mut self) {
        self.collect();
    }
}

macro_rules! define_resources {
    ($($name:ident: $info:ty,)*) => {
        #[derive(Debug)]
        pub enum Garbage<B: Backend> {
            $( $name(B::$name), )*
        }

        #[derive(Clone)]
        pub enum Any<B: Backend> {
            $( $name(self::raw::$name<B>), )*
        }

        pub mod inner {
            use Backend;
            use super::{Garbage, GarbageSender};
            use std::{cmp, hash};

            $(

            #[derive(Debug)]
            pub struct $name<B: Backend> {
                // option for owned drop
                resource: Option<B::$name>,
                info: $info,
                garbage: Option<GarbageSender<B>>,
            }

            impl<B: Backend> $name<B> {
                pub(crate) fn new(
                    resource: B::$name,
                    info: $info,
                    garbage: GarbageSender<B>) -> Self
                {
                    $name {
                        resource: Some(resource),
                        info,
                        garbage: Some(garbage),
                    }
                }

                #[allow(unused)]
                pub(crate) fn without_garbage(
                    resource: B::$name,
                    info: $info
                ) -> Self {
                    $name {
                        resource: Some(resource),
                        info,
                        garbage: None,
                    }
                }

                pub fn resource(&self) -> &B::$name {
                    self.resource_info().0
                }

                pub fn info(&self) -> &$info {
                    self.resource_info().1
                }

                pub fn resource_info(&self) -> (&B::$name, &$info) {
                    (self.resource.as_ref().unwrap(),
                     &self.info)
                }
            }

            impl<B: Backend> cmp::PartialEq for $name<B>
                where B::$name: cmp::PartialEq
            {
                fn eq(&self, other: &$name<B>) -> bool {
                    self.resource().eq(&other.resource())
                }
            }

            impl<B: Backend> cmp::Eq for $name<B>
                where B::$name: cmp::Eq
            {}

            impl<B: Backend> hash::Hash for $name<B>
                where B::$name: hash::Hash
            {
                fn hash<H: hash::Hasher>(&self, state: &mut H) {
                    self.resource().hash(state)
                }
            }

            impl<B: Backend> Drop for $name<B> {
                fn drop(&mut self) {
                    let res = self.resource.take().unwrap();
                    self.garbage.as_mut().map(|sender| sender.send(Garbage::$name(res))
                        .unwrap_or_else(|e|
                            error!("Could not drop {}: {}", stringify!($name), e)));
                }
            }

            )*
        }

        pub mod raw {
            use std::{ops, cmp, hash};
            use std::sync::Arc;
            use Backend;
            use super::inner;
            $(
                #[derive(Debug, Clone)]
                pub struct $name<B: Backend>(Arc<inner::$name<B>>);

                impl<B: Backend> From<inner::$name<B>> for $name<B> {
                    fn from(inner: inner::$name<B>) -> Self {
                        $name(Arc::new(inner))
                    }
                }

                impl<B: Backend> From<$name<B>> for super::Any<B> {
                    fn from(h: $name<B>) -> Self {
                        super::Any::$name(h)
                    }
                }

                impl<B: Backend> ops::Deref for $name<B> {
                    type Target = inner::$name<B>;
                    fn deref(&self) -> &Self::Target { &self.0 }
                }

                impl<B: Backend> $name<B> {
                    fn as_ptr(&self) -> *const inner::$name<B> {
                        self.0.as_ref()
                    }
                }

                impl<B: Backend> cmp::PartialEq for $name<B> {
                    fn eq(&self, other: &$name<B>) -> bool {
                        self.as_ptr().eq(&other.as_ptr())
                    }
                }

                impl<B: Backend> cmp::Eq for $name<B> {}

                impl<B: Backend> hash::Hash for $name<B> {
                    fn hash<H: hash::Hasher>(&self, state: &mut H) {
                        self.as_ptr().hash(state)
                    }
                }

                impl<B: Backend> AsRef<$name<B>> for $name<B> {
                    fn as_ref(&self) -> &$name<B> { self }
                }
            )*
        }
    }
}

define_resources! {
    // ShaderLib,
    RenderPass: (),
    PipelineLayout: (),
    GraphicsPipeline: (),
    // ComputePipeline
    Framebuffer: ::handle::FramebufferInfo<B>,
    Buffer: ::buffer::Info,
    BufferView: ::handle::raw::Buffer<B>,
    Image: ::image::Info,
    ImageView: ::handle::raw::Image<B>,
    Sampler: ::image::SamplerInfo,
    DescriptorPool: (),
    DescriptorSetLayout: (),
    // Fence
    // Semaphore
}

pub type Buffer<B, T> = Typed<raw::Buffer<B>, T>;
pub type BufferView<B, T> = Typed<raw::BufferView<B>, T>;
pub type Image<B, F> = Typed<raw::Image<B>, F>;
pub type ImageView<B, F> = Typed<raw::ImageView<B>, F>;

pub use self::raw::Sampler;

#[derive(Debug, Clone)]
pub struct FramebufferInfo<B: Backend> {
    pub attachments: Vec<raw::ImageView<B>>,
    pub extent: ::Extent,
}


pub struct Bag<B: Backend>(Vec<Any<B>>);

impl<B: Backend> Bag<B> {
    pub fn new() -> Self {
        Bag(Vec::new())
    }

    pub fn add<H: Into<Any<B>>>(&mut self, handle: H) {
        self.0.push(handle.into());
    }

    pub fn append(&mut self, other: &mut Bag<B>) {
        self.0.append(&mut other.0);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<'a, B: Backend, H: Into<Any<B>>> Extend<H> for Bag<B> {
    fn extend<I>(&mut self, iter: I)
        where I: IntoIterator<Item = H>
    {
        self.0.extend(iter.into_iter().map(|handle| handle.into()));
    }
}
