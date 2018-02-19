// #![deny(missing_docs)] TODO

// TODO(doc) clarify the different type of queues and what is accessible from the high-level API
// vs what belongs to core-ll. There doesn't seem to be a "ComputeEncoder" can I submit something
// built with a GraphicsEncoder to a ComputeQueue?

//! # gfx
//!
//! An efficient, low-level, bindless graphics API for Rust.
//!
//! # Overview
//!
//! ## Command buffers and encoders and queues
//!
//! A command buffer is a serialized list of drawing and compute commands.
//! Unlike with vulkan, command buffers are not what you use to create commands, but only
//! the result of creating these commands. Gfx, borrowing metal's terminology, uses
//! encoders to build command buffers. This means that, in general, users of the gfx crate
//! don't manipulate command buffers directly much and interact mostly with graphics encoders.
//! In order to be executed, a command buffer is then submitted to a queue.
//!
//! Manipulating a `GraphicsEncoder` in gfx corresponds to interacting with:
//!
//! - a `VkCommandBuffer` in vulkan,
//! - a `MTLCommandEncoder` in metal,
//! - an `ID3D12GraphicsCommandList` in D3D12.
//!
//! OpenGL and earlier versions of D3D don't have an explicit notion of command buffers
//! encoders or queues (with the exception of draw indirect commands in late versions of OpenGL,
//! which can be seen as a GPU-side command buffer). They are managed implicitly by the driver.
//!
//! See:
//!
//! - The [`GraphicsEncoder` struct](struct.GraphicsEncoder.html).
//! - The [`CommandBuffer` trait](trait.CommandBuffer.html).
//! - The [`CommandQueue` struct](struct.CommandQueue.html).
//!
//! ## Device
//!
//! The device is what lets you allocate GPU resources such as buffers and textures.
//!
//! Each gfx backend provides its own device type which implements both:
//!
//! - The [`Device` trait](traits/trait.Device.html#overview).
//! - The [`DeviceExt` trait](traits/trait.DeviceExt.html).
//!
//! `gfx::Device` is roughly equivalent to:
//!
//! - `VkDevice` in vulkan,
//! - `ID3D11Device` in D3D11,
//! - `MTLDevice` in metal.
//!
//! OpenGL does not have a notion of device (resources are created directly off of the global
//! context). D3D11 has a DXGI factory but it is only used to interface with other processes
//! and the window manager, resources like textures are usually created using the device.
//!
//! ## Gpu
//!
//! The `Gpu` contains the `Device` and the `Queue`s.
//!
//! ## Pipeline state (PSO)
//!
//! See [the documentation of the gfx::pso module](pso/index.html).
//!
//! ## Memory management
//!
//! Handles internally use atomically reference counted pointers to deal with memory management.
//! GPU resources are not destroyed right away when all references to them are gone. Instead they
//! are destroyed the next time `cleanup` is called on the queue.
//!
//! # Examples
//!
//! See [the examples in the repository](https://github.com/gfx-rs/gfx/tree/master/examples).
//!
//! # Useful resources
//!
//!  - [Documentation for some of the technical terms](doc/terminology/index.html)
//! used in the API.
//!  - [Learning gfx](https://wiki.alopex.li/LearningGfx) tutorial.
//!  - See [the blog](http://gfx-rs.github.io/) for more explanations and annotated examples.
//!

#[macro_use]
extern crate bitflags;

#[cfg(feature = "mint")]
extern crate mint;

#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde;

#[macro_use]
extern crate log;
extern crate failure;

pub extern crate gfx_hal as hal;

/// public re-exported traits
pub mod traits {
    pub use hal::memory::Pod;
}

// public re-exports
pub use hal::format;
pub use hal::{Backend, Frame, Primitive};
pub use hal::queue::{Supports, Transfer, General, Graphics};
pub use hal::{VertexCount, InstanceCount};
pub use hal::device::Extent;
// pub use hal::{ShaderSet, VertexShader, HullShader, DomainShader, GeometryShader, PixelShader};
pub use encoder::Encoder;
pub use device::Device;

pub mod handle;
mod device;
pub mod encoder;
pub mod memory;
pub mod allocators;
pub mod buffer;
pub mod image;
/// Pipeline states
pub mod pso;
/*
/// Shaders
pub mod shade;
*/
/// Convenience macros
pub mod macros;

use std::collections::VecDeque;
use hal::{
    Capability, CommandQueue, PhysicalDevice, Surface, Swapchain,
    Device as Device_,
};
use hal::format::AsFormat;
use hal::pool::CommandPoolCreateFlags;
use memory::Typed;

struct Queue<B: Backend, C> {
    group: hal::QueueGroup<B, C>,
    pool_receiver: encoder::CommandPoolReceiver<B, C>,
    pool_sender: encoder::CommandPoolSender<B, C>,
}

impl<B: Backend, C: hal::Capability> Queue<B, C> {
    fn new(group: hal::queue::QueueGroup<B, C>) -> Self {
        let (pool_sender, pool_receiver) =
            encoder::command_pool_channel();
        Queue { group, pool_sender, pool_receiver }
    }

    fn acquire_encoder_pool(
        &mut self, device: &B::Device
    ) -> encoder::Pool<B, C> {
        let pool = self.pool_receiver.try_recv()
            .map(|mut recycled| {
                recycled.reset();
                recycled
            })
            .unwrap_or_else(|_| {
                let initial_capacity = 4;
                let flags = CommandPoolCreateFlags::empty();
                device.create_command_pool_typed(&self.group, flags, initial_capacity)
            });
        encoder::Pool::new(pool, self.pool_sender.clone())
    }
}

pub struct Context<B: Backend, C> {
    surface: B::Surface,
    device: Device<B>,
    queue: Queue<B, C>,
    swapchain: B::Swapchain,
    frame_bundles: VecDeque<FrameBundle<B, C>>,
    frame_acquired: Option<FrameBundle<B, C>>,
    garbage: handle::GarbageCollector<B>,
}

pub struct Backbuffer<B: Backend, Cf: AsFormat> {
    pub color: handle::Image<B, Cf>,
}

use self::Signal::*;
#[derive(PartialEq)]
enum Signal {
    // A signal is pending and can be waited for
    Pending,
    // No signal is pending or it is already waited for
    Reached,
}

struct Sync<T> {
    inner: T,
    signal: Signal,
}

impl<T> Sync<T> {
    fn reached(inner: T) -> Self {
        Sync { inner, signal: Reached }
    }
}

struct FrameBundle<B: Backend, C> {
    handles: handle::Bag<B>,
    access_info: encoder::AccessInfo<B>,
    encoder_pools: Vec<encoder::PoolDependency<B, C>>,
    // wait until the backbuffer image is ready
    wait_semaphore: B::Semaphore,
    // signal when the frame is done
    signal_semaphore: B::Semaphore,
    signal_fence: Sync<B::Fence>,
}

impl<B: Backend, C> Context<B, C>
    where C: Capability + Supports<Transfer>
{
    pub fn init<Cf>(
        mut surface: B::Surface, adapter: hal::Adapter<B>
    ) -> Result<(Self, Vec<Backbuffer<B, Cf>>), failure::Error>
    where
        Cf: AsFormat,
    {
        let memory_properties = adapter.physical_device.memory_properties();
        let (device, queues) = adapter.open_with(1, |family| {
            surface.supports_queue_family(family)
        })?;

        let queue = Queue::new(queues);

        let swap_config = hal::SwapchainConfig::new()
            .with_color(Cf::SELF); // TODO: check support
        let (swapchain, backbuffer) = device.create_swapchain(&mut surface, swap_config);

        let backbuffer_images = match backbuffer {
            hal::Backbuffer::Images(images) => images,
            hal::Backbuffer::Framebuffer(_) => unimplemented!(), //TODO
        };

        let frame_bundles = backbuffer_images
            .iter()
            .map(|_| FrameBundle {
                handles: handle::Bag::new(),
                access_info: encoder::AccessInfo::new(),
                encoder_pools: Vec::new(),
                wait_semaphore: device.create_semaphore(),
                signal_semaphore: device.create_semaphore(),
                signal_fence: Sync::reached(
                    device.create_fence(true)),
            }).collect();

        let backbuffers = backbuffer_images
            .into_iter()
            .map(|raw| {
                let stable_access = hal::image::Access::empty();
                let stable_layout = hal::image::ImageLayout::Present;
                let handle = handle::inner::Image::without_garbage(
                    raw,
                    image::Info {
                        aspects: format::AspectFlags::COLOR,
                        usage: image::Usage::TRANSFER_SRC | image::Usage::COLOR_ATTACHMENT,
                        kind: surface.kind(),
                        mip_levels: 1,
                        format: Cf::SELF,
                        origin: image::Origin::Backbuffer,
                        stable_state: (stable_access, stable_layout),
                    },
                );
                Backbuffer {
                    color: Typed::new(handle.into()),
                }
            }).collect();

        let (device, garbage) = Device::new(
            device,
            memory_properties.memory_types,
            memory_properties.memory_heaps,
        );

        let context = Context {
            surface,
            device,
            queue,
            swapchain,
            frame_bundles,
            frame_acquired: None,
            garbage,
        };

        Ok((context, backbuffers))
    }

    pub fn acquire_frame(&mut self) -> Frame {
        assert!(self.frame_acquired.is_none());

        let mut bundle = self.frame_bundles.pop_front()
            .expect("no frame bundles");

        if bundle.signal_fence.signal == Pending {
            self.device.raw.wait_for_fence(&bundle.signal_fence.inner, !0);
        }
        self.device.raw.reset_fence(&bundle.signal_fence.inner);
        bundle.signal_fence.signal = Reached;

        bundle.handles.clear();
        bundle.access_info.end_gpu_access();
        bundle.access_info.clear();
        bundle.encoder_pools.clear();

        let frame = self.swapchain.acquire_frame(
            hal::FrameSync::Semaphore(&mut bundle.wait_semaphore)
        );
        self.frame_acquired = Some(bundle);

        self.garbage.collect();
        frame
    }

    pub fn acquire_encoder_pool(&mut self) -> encoder::Pool<B, C> {
        self.queue.acquire_encoder_pool(&self.device.raw)
    }

    // TODO: allow submissions before present
    pub fn present(&mut self, submits: Vec<encoder::Submit<B, C>>) {
        let mut bundle = self.frame_acquired.take()
            .expect("no acquired frame");

        let inner_submits: Vec<_> = submits.into_iter()
            .map(|mut submit| {
                bundle.handles.append(&mut submit.handles);
                bundle.access_info.append(&mut submit.access_info);
                bundle.encoder_pools.push(submit.pool);
                submit.inner
            }).collect();

        bundle.access_info.start_gpu_access();

        {
            let submission = hal::Submission::new()
                .wait_on(&[(&bundle.wait_semaphore, hal::pso::PipelineStage::BOTTOM_OF_PIPE)])
                .signal(&[&bundle.signal_semaphore])
                .promote::<C>()
                .submit(inner_submits);
            let fence = Some(&bundle.signal_fence.inner);
            self.queue.group.queues[0].submit::<C>(submission, fence);
        }
        bundle.signal_fence.signal = Pending;

        self.swapchain.present(
            &mut self.queue.group.queues[0],
            Some(&bundle.signal_semaphore),
        );

        self.frame_bundles.push_back(bundle);
    }
}

impl<B: Backend, C> Context<B, C> {
    fn wait_idle(&mut self) {
        assert!(self.frame_acquired.is_none());

        // TODO?: WaitIdle on queue instead
        let fences = self.frame_bundles.iter_mut()
            .filter_map(|bundle| {
                // self can drop the handles before waiting because
                // self will be the one receiving the garbage afterwards
                bundle.handles.clear();
                bundle.encoder_pools.clear();
                if bundle.signal_fence.signal == Pending {
                    Some(&bundle.signal_fence.inner)
                } else {
                    None
                }
            });

        self.device.raw.wait_for_fences(fences, hal::device::WaitFor::All, !0);
    }

    pub fn ref_device(&self) -> &Device<B> {
        &self.device
    }

    pub fn mut_device(&mut self) -> &mut Device<B> {
        &mut self.device
    }

    // TODO: remove
    pub fn mut_queue(&mut self) -> &mut CommandQueue<B, C> {
        &mut self.queue.group.queues[0]
    }
}

impl<B: Backend, C> Drop for Context<B, C> {
    fn drop(&mut self) {
        let _ = &self.surface;
        self.wait_idle();
        self.garbage.collect();

        for bundle in self.frame_bundles.drain(..) {
            self.device.raw.destroy_semaphore(bundle.wait_semaphore);
            self.device.raw.destroy_semaphore(bundle.signal_semaphore);
            self.device.raw.destroy_fence(bundle.signal_fence.inner);
        }
    }
}
