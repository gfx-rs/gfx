use std::marker::PhantomData;
use std::{ops, cmp, hash};
use std::sync::mpsc;

use Backend;

pub(crate) type GarbageSender<B> = mpsc::Sender<Garbage<B>>;
pub(crate) type GarbageReceiver<B> = mpsc::Receiver<Garbage<B>>;
pub(crate) fn garbage_channel<B: Backend>() -> (GarbageSender<B>, GarbageReceiver<B>) {
    mpsc::channel()
}

macro_rules! define_resources {
    ($($name:ident: $info:path,)*) => {
        pub enum Garbage<B: Backend> {
            $( $name(B::$name), )*
        }

        pub mod inner {
            use Backend;
            use super::{Garbage, GarbageSender};
            use std::{cmp, hash};

            $(
            
            #[derive(Clone, Debug)]
            pub struct $name<B: Backend> {
                resource: Option<B::$name>,
                info: $info,
                garbage: GarbageSender<B>
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
                        garbage,
                    }
                }

                pub fn resource(&self) -> &B::$name {
                    self.resource.as_ref().unwrap()
                }

                pub fn info(&self) -> &$info {
                    &self.info
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
                    self.garbage.send(Garbage::$name(res))
                        .unwrap_or_else(|e|
                            error!("Could not drop {}: {}", stringify!($name), e));
                }
            }

            )*
        }

        pub mod raw {
            use std::sync::Arc;
            $( pub type $name<B> = Arc<super::inner::$name<B>>; )*
        }
    }
}

define_resources! {
    // Heap
    // ShaderLib,
    // RenderPass
    // PipelineLayout
    // GraphicsPipeline
    // ComputePipeline
    // FrameBuffer
    Buffer: ::buffer::Info,
    Image: ::image::Info,
    // RenderTargetView,
    // DepthStencilView,
    // ConstantBufferView,
    // ShaderResourceView,
    // UnorderedAccessView,
    // Sampler,
    // DescriptorPool
    // DescriptorSetLayout
    // Fence
    // Semaphore
}

pub type Buffer<B, T> = Typed<raw::Buffer<B>, T>;
pub type Image<B, S> = Typed<raw::Image<B>, S>;

#[derive(Debug)]
pub struct Typed<I, T> {
    inner: I,
    phantom: PhantomData<T>,
}

impl<I, T> Typed<I, T> {
    pub fn new(inner: I) -> Self {
        Typed {
            inner,
            phantom: PhantomData,
        }
    }
}

impl<I: Clone, T> Clone for Typed<I, T> {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<I, T> cmp::PartialEq for Typed<I, T>
    where I: cmp::PartialEq
{
    fn eq(&self, other: &Typed<I, T>) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<I, T> cmp::Eq for Typed<I, T>
    where I: cmp::Eq
{}

impl<I, T> hash::Hash for Typed<I, T>
    where I: hash::Hash
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<I, T> ops::Deref for Typed<I, T> {
    type Target = I;

    fn deref(&self) -> &I {
        &self.inner
    }
}

impl<I, T> ops::DerefMut for Typed<I, T> {
    fn deref_mut(&mut self) -> &mut I {
        &mut self.inner
    }
}
