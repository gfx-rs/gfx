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
//! ## Devoce
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
extern crate draw_state;
extern crate gfx_core as core;

/// public re-exported traits
pub mod traits {
    pub use core::memory::Pod;
}

// draw state re-exports
pub use draw_state::{preset, state};
pub use draw_state::target::*;

// public re-exports
pub use core::{format, pso};
pub use core::{Adapter, Backend, Primitive, Frame};
/*
pub use core::{VertexCount, InstanceCount};
pub use core::{ShaderSet, VertexShader, HullShader, DomainShader, GeometryShader, PixelShader};
pub use core::device::{ResourceViewError, TargetViewError, CombinedError, WaitFor};
pub use core::command::{InstanceParams};
pub use core::shade::{ProgramInfo, UniformValue};

pub use encoder::{CopyBufferResult, CopyBufferTextureResult, CopyError,
                  CopyTextureBufferResult, UpdateError};
*/
pub use device::Device;
/*
pub use pso::{PipelineState};
pub use pso::buffer::{VertexBuffer, InstanceBuffer, RawVertexBuffer,
                      ConstantBuffer, RawConstantBuffer, Global, RawGlobal};
pub use pso::resource::{ShaderResource, RawShaderResource, UnorderedAccess,
                        Sampler, TextureSampler};
pub use pso::target::{DepthStencilTarget, DepthTarget, StencilTarget,
                      RenderTarget, RawRenderTarget, BlendTarget, BlendRef, Scissor};
pub use pso::bundle::{Bundle};
*/

pub mod handle;
mod device;
pub mod encoder;
pub mod memory;
pub mod allocators;
pub mod buffer;
pub mod image;
pub mod mapping;
/*
// Pipeline states
pub mod pso;
/// Shaders
pub mod shade;
/// Convenience macros
pub mod macros;
*/

use std::collections::VecDeque;
use core::{CommandQueue, QueueType, Surface, Swapchain, Device as CoreDevice};
use core::pool::{CommandPool, CommandPoolCreateFlags};
use core::format::RenderFormat;
use memory::Typed;

struct Queue<B: Backend, C> {
    inner: CommandQueue<B, C>,
    command_pool_receiver: encoder::CommandPoolReceiver<B, C>,
    command_pool_sender: encoder::CommandPoolSender<B, C>,
}

impl<B: Backend, C> Queue<B, C> {
    fn new(inner: CommandQueue<B, C>) -> Self {
        let (command_pool_sender, command_pool_receiver) =
            encoder::command_pool_channel();
        Queue { inner, command_pool_sender, command_pool_receiver }
    }

    fn acquire_encoder_pool(&mut self) -> encoder::Pool<B, C> {
        let initial_capacity = 4;
        let flags = CommandPoolCreateFlags::empty();
        let pool = self.command_pool_receiver.try_recv()
            .map(|mut recycled| {
                recycled.reset();
                recycled
            })
            .unwrap_or_else(|_| {
                CommandPool::from_queue(&self.inner, initial_capacity, flags)
            });
        encoder::Pool::new(pool, self.command_pool_sender.clone())
    }
}

pub struct Context<B: Backend, C>
    where C: core::queue::Supports<core::Transfer>
{
    surface: B::Surface,
    device: Device<B>,
    queue: Queue<B, C>,
    swapchain: B::Swapchain,
    frame_bundles: VecDeque<FrameBundle<B, C>>,
    frame_acquired: Option<FrameBundle<B, C>>,
    garbage: handle::GarbageCollector<B>,
}

pub struct Backbuffer<B: Backend, Cf: RenderFormat> {
    pub color: handle::RenderTargetView<B, Cf>,
    // TODO: depth
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

trait Capability: Sized {
    fn open<B: Backend>(
        surface: &B::Surface,
        adapter: &B::Adapter,
    ) -> (Device<B>, Queue<B, Self>, handle::GarbageCollector<B>);
}

impl Capability for core::General {
    fn open<B: Backend>(
        surface: &B::Surface,
        adapter: &B::Adapter,
    ) -> (Device<B>, Queue<B, Self>, handle::GarbageCollector<B>) {
        let core::Gpu {
            device,
            mut general_queues,
            memory_types,
            memory_heaps,
            ..
        } = adapter.open_with(|ref family, qtype| {
            if qtype.supports_graphics()
               && qtype.supports_compute()
               && surface.supports_queue(family) {
                (1, QueueType::General)
            } else {
                (0, QueueType::Transfer)
            }
        });

        let (device, garbage) = Device::new(device, memory_types, memory_heaps);
        let queue = Queue::new(general_queues.remove(0));
        (device, queue, garbage)
    }
}

impl Capability for core::Graphics {
    fn open<B: Backend>(
        surface: &B::Surface,
        adapter: &B::Adapter,
    ) -> (Device<B>, Queue<B, Self>, handle::GarbageCollector<B>) {
        let core::Gpu {
            device,
            mut graphics_queues,
            memory_types,
            memory_heaps,
            ..
        } = adapter.open_with(|ref family, qtype| {
            if qtype.supports_graphics() && surface.supports_queue(family) {
                (1, QueueType::Graphics)
            } else {
                (0, QueueType::Transfer)
            }
        });

        let (device, garbage) = Device::new(device, memory_types, memory_heaps);
        let queue = Queue::new(graphics_queues.remove(0));
        (device, queue, garbage)
    }
}

impl<B: Backend> Context<B, core::General> {
    pub fn init_general<Cf>(
        surface: B::Surface,
        adapter: &B::Adapter
    ) -> (Self, Vec<Backbuffer<B, Cf>>)
        where Cf: RenderFormat
    {
        Context::init(surface, adapter)
    }
}

impl<B: Backend> Context<B, core::Graphics> {
    pub fn init_graphics<Cf>(
        surface: B::Surface,
        adapter: &B::Adapter
    ) -> (Self, Vec<Backbuffer<B, Cf>>)
        where Cf: RenderFormat
    {
        Context::init(surface, adapter)
    }
}

impl<B: Backend, C> Context<B, C>
    where C: core::queue::Supports<core::Transfer>
{
    fn init<Cf>(mut surface: B::Surface, adapter: &B::Adapter)
        -> (Self, Vec<Backbuffer<B, Cf>>)
        where Cf: RenderFormat, C: Capability
    {
        let (mut device, queue, garbage) = Capability::open(&surface, adapter);

        let swap_config = core::SwapchainConfig::new()
            .with_color::<Cf>();
        let (swapchain, backbuffer) = surface.build_swapchain(swap_config, &queue.inner);

        let backbuffer_images = match backbuffer {
            core::Backbuffer::Images(images) => images,
            core::Backbuffer::FrameBuffer(_) => unimplemented!(), //TODO
        };

        let frame_bundles = backbuffer_images
            .iter()
            .map(|_| FrameBundle {
                handles: handle::Bag::new(),
                access_info: encoder::AccessInfo::new(),
                encoder_pools: Vec::new(),
                wait_semaphore: device.mut_raw().create_semaphore(),
                signal_semaphore: device.mut_raw().create_semaphore(),
                signal_fence: Sync::reached(
                    device.mut_raw().create_fence(true)),
            }).collect();

        let backbuffers = backbuffer_images
            .into_iter()
            .map(|raw| {
                Backbuffer {
                    color: Typed::new(device
                        .view_backbuffer_as_render_target_raw(
                            raw,
                            surface.get_kind(),
                            Cf::get_format(),
                            (0, 0..1)
                        ).expect("backbuffer RTV")
                    )
                }
            }).collect();

        (Context {
            surface,
            device,
            queue,
            swapchain,
            frame_bundles,
            frame_acquired: None,
            garbage,
        }, backbuffers)
    }

    pub fn acquire_frame(&mut self) -> Frame {
        assert!(self.frame_acquired.is_none());

        let mut bundle = self.frame_bundles.pop_front()
            .expect("no frame bundles");

        if bundle.signal_fence.signal == Pending {
            self.device.mut_raw()
                .wait_for_fences(
                    &[&bundle.signal_fence.inner],
                    core::device::WaitFor::All,
                    !0);
        }
        self.device.mut_raw().reset_fences(&[&bundle.signal_fence.inner]);
        bundle.signal_fence.signal = Reached;

        bundle.handles.clear();
        bundle.access_info.end_gpu_access();
        bundle.access_info.clear();
        bundle.encoder_pools.clear();

        let frame = self.swapchain.acquire_frame(
            core::FrameSync::Semaphore(&mut bundle.wait_semaphore)
        );
        self.frame_acquired = Some(bundle);

        self.garbage.collect();
        frame
    }

    pub fn acquire_encoder_pool(&mut self) -> encoder::Pool<B, C> {
        self.queue.acquire_encoder_pool()
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

        assert!(bundle.access_info.start_gpu_access()); // TODO: recovery

        {
            let submission = core::Submission::new()
                .wait_on(&[(&bundle.wait_semaphore, pso::BOTTOM_OF_PIPE)])
                .signal(&[&bundle.signal_semaphore])
                .promote::<C>()
                .submit(&inner_submits);
            self.queue.inner.submit::<C>(submission, Some(&bundle.signal_fence.inner));
        }
        bundle.signal_fence.signal = Pending;

        self.swapchain.present(
            &mut self.queue.inner,
            &[&bundle.signal_semaphore]);

        self.frame_bundles.push_back(bundle);
    }

    fn wait_idle(&mut self) {
        assert!(self.frame_acquired.is_none());

        // TODO?: WaitIdle on queue instead
        let fences: Vec<_> = self.frame_bundles.iter_mut()
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
            }).collect();

        self.device.mut_raw()
            .wait_for_fences(&fences, core::device::WaitFor::All, !0);
    }

    pub fn ref_device(&self) -> &Device<B> {
        &self.device
    }

    pub fn mut_device(&mut self) -> &mut Device<B> {
        &mut self.device
    }

    // TODO: remove
    pub fn mut_queue(&mut self) -> &mut CommandQueue<B, C> {
        &mut self.queue.inner
    }
}

impl<B: Backend, C> Drop for Context<B, C>
    where C: core::queue::Supports<core::Transfer>
{
    fn drop(&mut self) {
        self.wait_idle();
        self.garbage.collect();

        let device = self.device.mut_raw();
        for bundle in self.frame_bundles.drain(..) {
            device.destroy_semaphore(bundle.wait_semaphore);
            device.destroy_semaphore(bundle.signal_semaphore);
            device.destroy_fence(bundle.signal_fence.inner);
        }
    }
}
