#[macro_use]
extern crate log;
#[macro_use]
extern crate objc;
extern crate objc_foundation;
extern crate cocoa;
extern crate gfx_core as core;
extern crate metal_rs as metal;
extern crate bit_set;
extern crate block;

// use cocoa::base::{selector, class};
// use cocoa::foundation::{NSUInteger};

use metal::*;

use block::{Block, ConcreteBlock};
use core::{handle, texture as tex};
use core::{QueueType, SubmissionResult};
use core::memory::{self, Usage, Bind};
use core::command::{AccessInfo, AccessGuard};

use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
// use std::{mem, ptr};

mod command;
mod device;
mod encoder;
mod mirror;
mod map;
pub mod native;
mod pool;

pub use self::device::Device;
pub use self::map::*;

// Grabbed from https://developer.apple.com/metal/limits/
const MTL_MAX_TEXTURE_BINDINGS: usize = 128;
const MTL_MAX_BUFFER_BINDINGS: usize = 31;
const MTL_MAX_SAMPLER_BINDINGS: usize = 16;

pub type ShaderModel = u16;

pub fn enumerate_adapters() -> Vec<Adapter> {
    // TODO: enumerate all devices
    let device = metal::create_system_default_device(); // Returns retained

    vec![
        Adapter {
            device,
            adapter_info: core::AdapterInfo {
                name: device.name().into(),
                vendor: 0,
                device: 0,
                software_rendering: false,
            },
            queue_families: [(QueueFamily, QueueType::General)],
        }
    ]
}

#[derive(Clone)]
pub struct QueueFamily;
impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 { 1 }
}

pub struct Adapter {
    device: MTLDevice,
    adapter_info: core::AdapterInfo,
    queue_families: [(QueueFamily, QueueType); 1],
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, QueueType, u32)])
        -> core::Gpu<Backend>
    {
        // Single queue family supported only
        assert_eq!(queue_descs.len(), 1);

        // Ascending order important here to get the best feature set
        use metal::MTLFeatureSet::*;
        let feature_set = [
            iOS_GPUFamily1_v1,
            iOS_GPUFamily1_v2,
            iOS_GPUFamily1_v3,
            iOS_GPUFamily1_v4,

            iOS_GPUFamily2_v1,
            iOS_GPUFamily2_v2,
            iOS_GPUFamily2_v3,
            iOS_GPUFamily2_v4,

            iOS_GPUFamily3_v1,
            iOS_GPUFamily3_v2,
            iOS_GPUFamily3_v3,

            tvOS_GPUFamily1_v1,
            tvOS_GPUFamily1_v2,
            tvOS_GPUFamily1_v3,

            macOS_GPUFamily1_v1,
            macOS_GPUFamily1_v2,
            macOS_GPUFamily1_v3,
        ].iter()
         .rev()
         .cloned()
         .find(|&f| self.device.supports_feature_set(f));

        let share = Share {
            capabilities: core::Capabilities {
                max_texture_size: 0,
                max_patch_size: 0,
                instance_base_supported: false,
                instance_call_supported: false,
                instance_rate_supported: false,
                vertex_base_supported: false,
                srgb_color_supported: false,
                constant_buffer_supported: true,
                unordered_access_view_supported: false,
                separate_blending_slots_supported: false,
                copy_buffer_supported: true,
            },
            handles: RefCell::new(handle::Manager::new()),
            feature_set: feature_set.unwrap(),
        };

        unsafe { self.device.retain(); }
        let device = Device::new(self.device, Arc::new(share));

        let mut gpu = core::Gpu {
            device,
            general_queues: Vec::new(),
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types: Vec::new(),
            memory_heaps: Vec::new(),
        };

        let raw_queue = || {
            unsafe { self.device.retain(); }
            CommandQueue::new(self.device)
        };

        if let Some(&(_, queue_type, queue_count)) = queue_descs.iter().next() {
            for _ in 0..queue_count {
                unsafe {
                    match queue_type {
                        QueueType::General => {
                            gpu.general_queues.push(core::GeneralQueue::new(raw_queue()));
                        }
                        QueueType::Graphics => {
                            gpu.graphics_queues.push(core::GraphicsQueue::new(raw_queue()));
                        }
                        QueueType::Compute => {
                            gpu.compute_queues.push(core::ComputeQueue::new(raw_queue()));
                        }
                        QueueType::Transfer => {
                            gpu.transfer_queues.push(core::TransferQueue::new(raw_queue()));
                        }
                    }
                }
            }
        }

        gpu
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.adapter_info
    }

    fn get_queue_families(&self) -> &[(QueueFamily, QueueType)] {
        &self.queue_families
    }
}

pub struct CommandQueue {
    raw: Arc<QueueInner>,
    frame_handles: handle::Manager<Resources>,
    max_resource_count: Option<usize>,
}

struct QueueInner {
    queue: MTLCommandQueue,
}

impl Drop for QueueInner {
    fn drop(&mut self) {
        unsafe {
            self.queue.release();
        }
    }
}

impl CommandQueue {
    pub fn new(device: MTLDevice) -> CommandQueue {
        let raw_queue = QueueInner { queue: device.new_command_queue() };
        CommandQueue {
            raw: Arc::new(raw_queue),
            frame_handles: handle::Manager::new(),
            max_resource_count: Some(999999),
        }
    }

    pub unsafe fn device(&self) -> MTLDevice {
        // TODO: How often do we call this and how costly is it?
        msg_send![self.raw.queue.0, device]
    }
}

// Iterator extension which checks
// if are at the last element of the iterator
trait CheckLast: Iterator + Sized {
    fn check_last(mut self) -> CheckLastIter<Self> {
        CheckLastIter {
            next: self.next(),
            iterator: self,
        }
    }
}

impl<It> CheckLast for It where It: Iterator { }

struct CheckLastIter<It: Iterator> {
    iterator: It,
    next: Option<It::Item>,
}

impl<It> Iterator for CheckLastIter<It> where It: Iterator {
    type Item = (bool, It::Item);
    fn next(&mut self) -> Option<Self::Item> {
        match self.next.take() {
            Some(cur_elem) => {
                match self.iterator.next() {
                    Some(next) => {
                        self.next = Some(next);
                        Some((false, cur_elem))
                    }
                    None => {
                        self.next = None;
                        Some((true, cur_elem))
                    }
                }
            }
            None => None,
        }
    }
}

impl core::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<'a, I>(
        &mut self,
        submit_infos: I,
        fence: Option<&handle::Fence<Resources>>,
        access: &AccessInfo<Resources>,
    ) where I: Iterator<Item=core::RawSubmission<'a, Backend>> {
        for (last_submission, submit) in submit_infos.check_last() {
            // FIXME: wait for semaphores!

            // FIXME: multiple buffers signaling!
            let signal_block = if !submit.signal_semaphores.is_empty() {
                let semaphores_copy: Vec<_> = submit.signal_semaphores.iter().map(|semaphore| {
                    self.frame_handles.ref_semaphore(semaphore).lock().unwrap().0
                }).collect();
                Some(ConcreteBlock::new(move |cb: *mut ()| -> () {
                    for semaphore in semaphores_copy.iter() {
                        native::dispatch_semaphore_signal(*semaphore);
                    }
                }).copy())
            } else {
                None
            };

            for (last_buffer, buffer) in submit.cmd_buffers.iter().check_last() {
                let command_buffer = buffer.command_buffer;
                if let Some(ref signal_block) = signal_block {
                    msg_send![command_buffer.0, addCompletedHandler: signal_block.deref() as *const _];
                }
                // only append the fence handler to the last command buffer
                if last_submission && last_buffer {
                    if let Some(ref fence) = fence {
                        let value_ptr = self.frame_handles.ref_fence(fence).lock().unwrap().0.clone();
                        let fence_block = ConcreteBlock::new(move |cb: *mut ()| -> () {
                            *value_ptr.lock().unwrap() = true;
                        }).copy();
                        msg_send![command_buffer.0, addCompletedHandler: fence_block.deref() as *const _];
                    }
                }
                command_buffer.commit();
            }
        }
    }

    fn pin_submitted_resources(&mut self, man: &handle::Manager<Resources>) {
        self.frame_handles.extend(man);
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn cleanup(&mut self) {
        use core::handle::Producer;
        self.frame_handles.clear();
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Adapter = Adapter;
    type Resources = Resources;
    type CommandQueue = CommandQueue;
    type RawCommandBuffer = command::RawCommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;
    type SubmitInfo = command::SubmitInfo;
    type Device = Device;
    type QueueFamily = QueueFamily;

    type RawCommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}
impl core::Resources for Resources {
    type Buffer = native::Buffer;
    type Shader = native::Shader;
    type Program = native::Program;
    type PipelineStateObject = native::Pipeline;
    type Texture = native::Texture;
    type ShaderResourceView = native::Srv;
    type UnorderedAccessView = native::Uav;
    type RenderTargetView = native::Rtv;
    type DepthStencilView = native::Dsv;
    type Sampler = native::Sampler;
    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type Mapping = device::RawMapping;
}

/// Internal struct of shared data.
#[doc(hidden)]
pub struct Share {
    capabilities: core::Capabilities,
    handles: RefCell<handle::Manager<Resources>>,
    feature_set: MTLFeatureSet,
}
