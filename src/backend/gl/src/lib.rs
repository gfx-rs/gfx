//! OpenGL implementation of a device, striving to support OpenGL 2.0 with at
//! least VAOs, but using newer extensions when available.

#![allow(missing_docs, missing_copy_implementations)]

#[macro_use]
extern crate log;
extern crate gfx_gl as gl;
extern crate gfx_core as core;
extern crate smallvec;
#[cfg(feature = "glutin")]
extern crate glutin;

use std::rc::Rc;
use core as c;
use core::QueueType;

pub use self::device::Device;
pub use self::info::{Info, PlatformName, Version};

mod command;
mod conv;
mod device;
mod info;
mod native;
mod pool;
mod queue;
mod state;
mod window;

#[cfg(feature = "glutin")]
pub use window::glutin::{Headless, Surface, Swapchain};

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl c::Backend for Backend {
    type Adapter = Adapter;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type CommandQueue = queue::CommandQueue;
    type CommandBuffer = command::RawCommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;
    type QueueFamily = QueueFamily;

    type Memory = native::Memory;
    type CommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type FrameBuffer = native::FrameBuffer;

    type UnboundBuffer = device::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = device::UnboundImage;
    type Image = native::Image;
    type Sampler = native::FatSampler;

    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::TargetView;
    type DepthStencilView = native::TargetView;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    NoError,
    InvalidEnum,
    InvalidValue,
    InvalidOperation,
    InvalidFramebufferOperation,
    OutOfMemory,
    UnknownError,
}

impl Error {
    pub fn from_error_code(error_code: gl::types::GLenum) -> Error {
        match error_code {
            gl::NO_ERROR                      => Error::NoError,
            gl::INVALID_ENUM                  => Error::InvalidEnum,
            gl::INVALID_VALUE                 => Error::InvalidValue,
            gl::INVALID_OPERATION             => Error::InvalidOperation,
            gl::INVALID_FRAMEBUFFER_OPERATION => Error::InvalidFramebufferOperation,
            gl::OUT_OF_MEMORY                 => Error::OutOfMemory,
            _                                 => Error::UnknownError,
        }
    }
}

/// Internal struct of shared data between the device and its factories.
struct Share {
    context: gl::Gl,
    info: Info,
    features: c::Features,
    limits: c::Limits,
    private_caps: info::PrivateCaps,
}

impl Share {
    /// Fails during a debug build if the implementation's error flag was set.
    fn check(&self) -> Result<(), Error> {
        if cfg!(debug_assertions) {
            let gl = &self.context;
            let err = Error::from_error_code(unsafe { gl.GetError() });
            if err != Error::NoError {
                return Err(err)
            }
        }
        Ok(())
    }
}

pub struct Adapter {
    share: Rc<Share>,
    adapter_info: c::AdapterInfo,
    queue_family: [(QueueFamily, QueueType); 1],
}

impl Adapter {
    fn new<F>(fn_proc: F) -> Self where
        F: FnMut(&str) -> *const std::os::raw::c_void
    {
        let gl = gl::Gl::load_with(fn_proc);
        // query information
        let (info, features, limits, private_caps) = info::query_all(&gl);
        info!("Vendor: {:?}", info.platform_name.vendor);
        info!("Renderer: {:?}", info.platform_name.renderer);
        info!("Version: {:?}", info.version);
        info!("Shading Language: {:?}", info.shading_language);
        debug!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            debug!("- {}", *extension);
        }

        let adapter_info = c::AdapterInfo {
            name: info.platform_name.renderer.into(),
            vendor: 0, // TODO
            device: 0, // TODO
            software_rendering: false, // not always true ..
        };

        let queue_type = {
            use info::Requirement::{Core, Es};
            let compute_supported = info.is_supported(&[Core(4,3), Es(3, 1)]); // TODO: extension
            if compute_supported {
                QueueType::General
            } else {
                QueueType::Graphics
            }
        };

        // create the shared context
        let share = Share {
            context: gl,
            info,
            features,
            limits,
            private_caps,
        };
        if let Err(err) = share.check() {
            panic!("Error querying info: {:?}", err);
        }

        Adapter {
            share: Rc::new(share),
            adapter_info: adapter_info,
            queue_family: [(QueueFamily, queue_type)],
        }
    }
}

impl c::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, QueueType, u32)]) -> c::Gpu<Backend> {
        // initialize permanent states
        let gl = &self.share.context;
        if self.share.features.srgb_color {
            unsafe {
                gl.Enable(gl::FRAMEBUFFER_SRGB);
            }
        }
        unsafe {
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            if !self.share.info.version.is_embedded {
                gl.Enable(gl::PROGRAM_POINT_SIZE);
            }
        }

        // create main VAO and bind it
        let mut vao = 0;
        if self.share.private_caps.vertex_array {
            unsafe {
                gl.GenVertexArrays(1, &mut vao);
                gl.BindVertexArray(vao);
            }
        }

        if let Err(err) = self.share.check() {
            panic!("Error opening adapter: {:?}", err);
        }

        let memory_types = vec![
            core::MemoryType {
                id: 0,
                properties: c::memory::DEVICE_LOCAL,
                heap_index: 1,
            },
            core::MemoryType {
                id: 1,
                properties: c::memory::CPU_VISIBLE | c::memory::CPU_CACHED,
                heap_index: 0,
            },
            core::MemoryType {
                id: 2,
                properties: c::memory::CPU_VISIBLE | c::memory::COHERENT,
                heap_index: 0,
            },
        ];

        let mut gpu = c::Gpu {
            device: Device::new(self.share.clone()),
            general_queues: Vec::new(),
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            memory_types,
            memory_heaps: vec![!0, !0],
        };

        for &(_, queue_type, num_queues) in queue_descs {
            if num_queues == 0 {
                continue
            }
            assert_eq!(num_queues, 1);
            let raw_queue = queue::CommandQueue::new(&self.share, vao);
            match queue_type {
                QueueType::General => unsafe {
                    gpu.general_queues.push(c::CommandQueue::new(raw_queue));
                },
                QueueType::Graphics => unsafe {
                    gpu.graphics_queues.push(c::CommandQueue::new(raw_queue));
                },
                QueueType::Compute => unsafe {
                    gpu.compute_queues.push(c::CommandQueue::new(raw_queue));
                },
                QueueType::Transfer => unsafe {
                    gpu.transfer_queues.push(c::CommandQueue::new(raw_queue));
                },
            }
        }

        gpu
    }

    fn get_info(&self) -> &c::AdapterInfo {
        &self.adapter_info
    }

    fn get_queue_families(&self) -> &[(QueueFamily, QueueType)] {
        &self.queue_family
    }
}

pub struct QueueFamily;

impl c::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 { 1 }
}
