use {Backend};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::raw::{c_void, c_long, c_int};
use std::ptr;

use core::{self, image, pass, pso};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::*;
use objc;

pub struct QueueFamily {
}

#[derive(Debug)]
pub struct ShaderModule(pub MTLLibrary);

unsafe impl Send for ShaderModule {}
unsafe impl Sync for ShaderModule {}

#[derive(Debug)]
pub struct RenderPass {
    pub desc: MTLRenderPassDescriptor,
    pub attachments: Vec<pass::Attachment>,
}

unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}

#[derive(Debug)]
pub struct FrameBuffer(pub MTLRenderPassDescriptor);

unsafe impl Send for FrameBuffer {}
unsafe impl Sync for FrameBuffer {}

#[derive(Debug)]
pub struct PipelineLayout {}

#[derive(Debug)]
pub struct GraphicsPipeline {
    pub(crate) raw: MTLRenderPipelineState,
    pub(crate) primitive_type: MTLPrimitiveType,
}

unsafe impl Send for GraphicsPipeline {}
unsafe impl Sync for GraphicsPipeline {}

#[derive(Debug)]
pub struct ComputePipeline {}

#[derive(Debug)]
pub struct Image(pub MTLTexture);

unsafe impl Send for Image {}
unsafe impl Sync for Image {}

#[derive(Debug)]
pub struct ConstantBufferView {}

#[derive(Debug)]
pub struct ShaderResourceView(pub MTLTexture);

unsafe impl Send for ShaderResourceView {}
unsafe impl Sync for ShaderResourceView {}

#[derive(Debug)]
pub struct UnorderedAccessView {}

#[derive(Debug)]
pub struct RenderTargetView(pub MTLTexture);

unsafe impl Send for RenderTargetView {}
unsafe impl Sync for RenderTargetView {}

#[derive(Debug)]
pub struct DepthStencilView(pub MTLTexture);

unsafe impl Send for DepthStencilView {}
unsafe impl Sync for DepthStencilView {}

#[derive(Debug)]
pub struct Sampler(pub MTLSamplerState);

unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

#[derive(Debug)]
pub struct Semaphore(pub *mut c_void);

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Buffer(pub MTLBuffer);

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}


#[cfg(feature = "argument_buffer")]
#[derive(Debug)]
pub struct DescriptorPool {
    pub arg_buffer: MTLBuffer,
    pub total_size: NSUInteger,
    pub offset: NSUInteger,
}
#[cfg(feature = "argument_buffer")]
unsafe impl Send for DescriptorPool {}
#[cfg(feature = "argument_buffer")]
unsafe impl Sync for DescriptorPool {} //TEMP!

#[cfg(not(feature = "argument_buffer"))]
#[derive(Debug)]
pub struct DescriptorPool {}

impl core::DescriptorPool<Backend> for DescriptorPool {
    #[cfg(feature = "argument_buffer")]
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        layouts.iter().map(|layout| {
            let offset = self.offset;
            self.offset += layout.encoder.encoded_length();

            DescriptorSet {
                buffer: self.arg_buffer.clone(),
                offset,
                encoder: layout.encoder.clone(),
                stage_flags: layout.stage_flags,
            }
        }).collect()
    }

    #[cfg(not(feature = "argument_buffer"))]
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        layouts.iter().map(|layout| {
            let bindings = layout.bindings.iter().map(|layout| {
                let binding = match layout.ty {
                    pso::DescriptorType::Sampler => {
                        DescriptorSetBinding::Sampler((0..layout.count).map(|_| MTLSamplerState::nil()).collect())
                    },
                    pso::DescriptorType::SampledImage => {
                        DescriptorSetBinding::SampledImage((0..layout.count).map(|_| (MTLTexture::nil(), image::ImageLayout::General)).collect())
                    },
                    _ => unimplemented!(),
                };
                (layout.binding, binding)
            }).collect();

            let inner = DescriptorSetInner {
                layout: layout.bindings.clone(),
                bindings,
            };
            DescriptorSet {
                inner: Arc::new(Mutex::new(inner)),
            }
        }).collect()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[cfg(feature = "argument_buffer")]
#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub encoder: MTLArgumentEncoder,
    pub stage_flags: pso::ShaderStageFlags,
}
#[cfg(feature = "argument_buffer")]
unsafe impl Send for DescriptorSetLayout {}
#[cfg(feature = "argument_buffer")]
unsafe impl Sync for DescriptorSetLayout {}

#[cfg(not(feature = "argument_buffer"))]
#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub bindings: Vec<pso::DescriptorSetLayoutBinding>,
}

#[derive(Clone, Debug)]
#[cfg(feature = "argument_buffer")]
pub struct DescriptorSet {
    pub buffer: MTLBuffer,
    pub offset: NSUInteger,
    pub encoder: MTLArgumentEncoder,
    pub stage_flags: pso::ShaderStageFlags,
}
#[cfg(feature = "argument_buffer")]
unsafe impl Send for DescriptorSet {}
#[cfg(feature = "argument_buffer")]
unsafe impl Sync for DescriptorSet {}

#[derive(Clone, Debug)]
#[cfg(not(feature = "argument_buffer"))]
pub struct DescriptorSet {
    pub inner: Arc<Mutex<DescriptorSetInner>>,
}

#[cfg(not(feature = "argument_buffer"))]
#[derive(Debug)]
pub struct DescriptorSetInner {
    pub layout: Vec<pso::DescriptorSetLayoutBinding>, // TODO: maybe don't clone?
    pub bindings: HashMap<usize, DescriptorSetBinding>,
}
#[cfg(not(feature = "argument_buffer"))]
unsafe impl Send for DescriptorSetInner {}

#[derive(Debug)]
pub enum DescriptorSetBinding {
    Sampler(Vec<MTLSamplerState>),
    SampledImage(Vec<(MTLTexture, image::ImageLayout)>),
    StorageImage(Vec<(MTLTexture, image::ImageLayout)>),
    UniformTexelBuffer,
    StorageTexelBuffer,
    ConstantBuffer(Vec<MTLBuffer>),
    StorageBuffer,
    InputAttachment(Vec<(MTLTexture, image::ImageLayout)>),
}

impl Drop for DescriptorSetBinding {
    fn drop(&mut self) {
        use self::DescriptorSetBinding::*;

        unsafe {
            match *self {
                Sampler(ref mut states) => for state in states.drain(..) {
                    state.release();
                },
                SampledImage(ref mut images) => for (image, _) in images.drain(..) {
                    image.release();
                },
                StorageImage(ref mut images) => for (image, _) in images.drain(..) {
                    image.release();
                },
                ConstantBuffer(ref mut buffers) => for buffer in buffers.drain(..) {
                    buffer.release();
                },
                InputAttachment(ref mut attachments) => for (attachment, _) in attachments.drain(..) {
                    attachment.release();
                },
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
pub enum Memory {
    Emulated { memory_type: core::MemoryType, size: u64 },
    Native(MTLHeap),
}

#[derive(Debug)]
pub struct UnboundBuffer {
    pub size: u64,
}

unsafe impl Send for UnboundBuffer {
}
unsafe impl Sync for UnboundBuffer {
}

#[derive(Debug)]
pub struct UnboundImage(pub MTLTextureDescriptor);

unsafe impl Send for UnboundImage {
}
unsafe impl Sync for UnboundImage {
}

#[derive(Debug)]
pub struct Fence(pub Arc<Mutex<bool>>);

impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 {
        1 // TODO: don't think there is a queue limit
    }
}

pub unsafe fn objc_err_description(object: *mut objc::runtime::Object) -> String {
    let description: *mut objc::runtime::Object = msg_send![object, localizedDescription];
    let utf16_len: NSUInteger = msg_send![description, length];
    let utf8_bytes: NSUInteger = msg_send![description, lengthOfBytesUsingEncoding: 4 as NSUInteger];
    let mut bytes = Vec::with_capacity(utf8_bytes as usize);
    bytes.set_len(utf8_bytes as usize);
    let success: objc::runtime::BOOL = msg_send![description, 
        getBytes: bytes.as_mut_ptr()
        maxLength: utf8_bytes
        usedLength: ptr::null_mut::<NSUInteger>()
        encoding: 4 as NSUInteger
        options: 0 as c_int
        range: NSRange  { location: 0, length: utf16_len }
        remainingRange: ptr::null_mut::<NSRange>()
    ];
    if success == objc::runtime::YES {
        String::from_utf8_unchecked(bytes)
    } else {
        panic!("failed to get object description")
    }
}

extern "C" {
    pub fn dispatch_semaphore_wait(
        semaphore: *mut c_void,
        timeout: u64,
    ) -> c_long;

    pub fn dispatch_semaphore_signal(
        semaphore: *mut c_void,
    ) -> c_long;

    pub fn dispatch_semaphore_create(
        value: c_long,
    ) -> *mut c_void;

    pub fn dispatch_release(
        object: *mut c_void,
    );
}

pub const kCVPixelFormatType_32RGBA: u32 = (b'R' as u32) << 24 | (b'G' as u32) << 16 | (b'B' as u32) << 8 | b'A' as u32;
