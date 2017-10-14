use {Backend};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::raw::{c_void, c_long, c_int};
use std::ptr;

use core::{self, image, pass, pso};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::*;
use objc;

pub struct QueueFamily {}

#[derive(Debug)]
pub struct ShaderModule(pub MTLLibrary);

unsafe impl Send for ShaderModule {}
unsafe impl Sync for ShaderModule {}

#[derive(Debug)]
pub struct RenderPass {
    pub(crate) desc: MTLRenderPassDescriptor,
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) num_colors: usize,
}

unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}

#[derive(Debug)]
pub struct FrameBuffer(pub(crate) MTLRenderPassDescriptor);

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
pub struct Image(pub(crate) MTLTexture);

unsafe impl Send for Image {}
unsafe impl Sync for Image {}

#[derive(Debug)]
pub struct BufferView {}

#[derive(Debug)]
pub struct ImageView(pub(crate) MTLTexture);

unsafe impl Send for ImageView {}
unsafe impl Sync for ImageView {}

#[derive(Debug)]
pub struct Sampler(pub(crate) MTLSamplerState);

unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

#[derive(Debug)]
pub struct Semaphore(pub(crate) *mut c_void);

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Buffer(pub(crate) MTLBuffer);

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}


#[derive(Debug)]
pub enum DescriptorPool {
    Emulated,
    ArgumentBuffer {
        buffer: MTLBuffer,
        total_size: NSUInteger,
        offset: NSUInteger,
    }
}
//TODO: re-evaluate Send/Sync here
unsafe impl Send for DescriptorPool {}
unsafe impl Sync for DescriptorPool {}

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        match *self {
            DescriptorPool::Emulated => {
                layouts.iter().map(|layout| {
                    let layout_bindings = match layout {
                        &&DescriptorSetLayout::Emulated(ref bindings) => bindings,
                        _ => panic!("Incompatible descriptor set layout type"),
                    };

                    let bindings = layout_bindings.iter().map(|layout| {
                        let binding = match layout.ty {
                            pso::DescriptorType::Sampler => {
                                let resources = (0 .. layout.count).map(|_| MTLSamplerState::nil());
                                DescriptorSetBinding::Sampler(resources.collect())
                            }
                            pso::DescriptorType::SampledImage => {
                                let resources = (0 .. layout.count)
                                    .map(|_| (MTLTexture::nil(), image::ImageLayout::General));
                                DescriptorSetBinding::SampledImage(resources.collect())
                            }
                            _ => unimplemented!()
                        };
                        (layout.binding, binding)
                    }).collect();

                    let inner = DescriptorSetInner {
                        layout: layout_bindings.to_vec(),
                        bindings,
                    };
                    DescriptorSet::Emulated(Arc::new(Mutex::new(inner)))
                }).collect()
            }
            DescriptorPool::ArgumentBuffer { buffer, total_size, ref mut offset } => {
                layouts.iter().map(|layout| {
                    let (encoder, stage_flags) = match layout {
                        &&DescriptorSetLayout::ArgumentBuffer(ref encoder, stages) => (encoder, stages),
                        _ => panic!("Incompatible descriptor set layout type"),
                    };

                    let cur_offset = *offset;
                    *offset += encoder.encoded_length();
                    assert!(*offset <= total_size);

                    DescriptorSet::ArgumentBuffer {
                        buffer: buffer.clone(),
                        offset: cur_offset,
                        encoder: encoder.clone(),
                        stage_flags,
                    }
                }).collect()
            }
        }
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub enum DescriptorSetLayout {
    Emulated(Vec<pso::DescriptorSetLayoutBinding>),
    ArgumentBuffer(MTLArgumentEncoder, pso::ShaderStageFlags),
}
unsafe impl Send for DescriptorSetLayout {}
unsafe impl Sync for DescriptorSetLayout {}

#[derive(Clone, Debug)]
pub enum DescriptorSet {
    Emulated(Arc<Mutex<DescriptorSetInner>>),
    ArgumentBuffer {
        buffer: MTLBuffer,
        offset: NSUInteger,
        encoder: MTLArgumentEncoder,
        stage_flags: pso::ShaderStageFlags,
    }
}
unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {}

#[derive(Debug)]
pub struct DescriptorSetInner {
    pub(crate) layout: Vec<pso::DescriptorSetLayoutBinding>, // TODO: maybe don't clone?
    pub(crate) bindings: HashMap<usize, DescriptorSetBinding>,
}
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
    pub(crate) size: u64,
}

unsafe impl Send for UnboundBuffer {}
unsafe impl Sync for UnboundBuffer {}

#[derive(Debug)]
pub struct UnboundImage(pub MTLTextureDescriptor);

unsafe impl Send for UnboundImage {}
unsafe impl Sync for UnboundImage {}

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
