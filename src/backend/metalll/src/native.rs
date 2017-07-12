use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::os::raw::{c_void, c_long, c_int};
use std::ptr;

use core;
use core::{format, memory};
use core::factory::DescriptorSetLayoutBinding;
use metal::*;
use objc;

pub use cocoa::foundation::NSUInteger;
pub use cocoa::foundation::NSRange;

#[derive(Debug)]
pub struct ShaderLib(pub MTLLibrary);

unsafe impl Send for ShaderLib {
}
unsafe impl Sync for ShaderLib {
}

#[derive(Debug)]
pub struct RenderPass(pub MTLRenderPassDescriptor);

unsafe impl Send for RenderPass {
}
unsafe impl Sync for RenderPass {
}

#[derive(Debug)]
pub struct FrameBuffer(pub MTLRenderPassDescriptor);

unsafe impl Send for FrameBuffer {
}
unsafe impl Sync for FrameBuffer {
}

#[derive(Debug)]
pub struct PipelineLayout {}

#[derive(Debug)]
pub struct GraphicsPipeline(pub MTLRenderPipelineState);

unsafe impl Send for GraphicsPipeline {
}
unsafe impl Sync for GraphicsPipeline {
}

#[derive(Debug)]
pub struct ComputePipeline {}

#[derive(Debug)]
pub struct Image(pub MTLTexture);

unsafe impl Send for Image {
}
unsafe impl Sync for Image {
}

#[derive(Debug)]
pub struct ConstantBufferView {}

#[derive(Debug)]
pub struct ShaderResourceView(pub MTLTexture);

unsafe impl Send for ShaderResourceView {
}
unsafe impl Sync for ShaderResourceView {
}

#[derive(Debug)]
pub struct UnorderedAccessView {}

#[derive(Debug)]
pub struct RenderTargetView(pub MTLTexture);

unsafe impl Send for RenderTargetView {
}
unsafe impl Sync for RenderTargetView {
}

#[derive(Debug)]
pub struct DepthStencilView(pub MTLTexture);

unsafe impl Send for DepthStencilView {
}
unsafe impl Sync for DepthStencilView {
}

#[derive(Debug)]
pub struct Sampler(pub MTLSamplerState);

unsafe impl Send for Sampler {
}
unsafe impl Sync for Sampler {
}

#[derive(Debug)]
pub struct Semaphore(pub *mut c_void);

unsafe impl Send for Semaphore {
}
unsafe impl Sync for Semaphore {
}

#[derive(Debug)]
pub struct Mapping(pub MappingInner);

pub enum MappingInner {
    Read,
    Write(MTLBuffer, NSRange),
}

impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe {
            if let MappingInner::Write(buffer, ref range) = self.0 {
                buffer.did_modify_range(NSRange {
                    location: range.location,
                    length: range.length,
                });
            }
        }
    }
}

impl fmt::Debug for MappingInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MappingInner::Read => write!(f, "Read"),
            MappingInner::Write(_, _) => write!(f, "Write"),
        }
    }
}

#[derive(Debug)]
pub struct Buffer(pub MTLBuffer);

unsafe impl Send for Buffer {
}
unsafe impl Sync for Buffer {
}

#[derive(Debug)]
pub struct DescriptorHeap {}
#[derive(Debug)]
pub struct DescriptorSetPool {}
#[derive(Debug)]
pub struct DescriptorSet(pub Arc<Mutex<DescriptorSetInner>>); // TODO: can only be modified via factory, might not need mutex?

#[derive(Debug)]
pub struct DescriptorSetInner {
    pub layout: Vec<DescriptorSetLayoutBinding>, // TODO: maybe don't clone?
    pub bindings: HashMap<usize, DescriptorSetBinding>,
}

#[derive(Debug)]
pub enum DescriptorSetBinding {
    Sampler(Vec<MTLSamplerState>),
    SampledImage(Vec<(MTLTexture, memory::ImageLayout)>),
    StorageImage(Vec<(MTLTexture, memory::ImageLayout)>),
    UniformTexelBuffer,
    StorageTexelBuffer,
    ConstantBuffer(Vec<MTLBuffer>),
    StorageBuffer,
    InputAttachment(Vec<(MTLTexture, memory::ImageLayout)>),
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
pub struct DescriptorSetLayout(pub Vec<DescriptorSetLayoutBinding>);


pub use self::heap_related::*;
pub use self::fence_related::*;

#[cfg(not(feature = "native_heap"))]
mod heap_related {
    use super::*;

    #[derive(Debug)]
    pub struct Heap {
        pub heap_type: core::HeapType,
        pub size: u64,
    }
    
    #[derive(Debug)]
    pub struct UnboundBuffer(pub MTLBuffer);

    unsafe impl Send for UnboundBuffer {
    }
    unsafe impl Sync for UnboundBuffer {
    }

    #[derive(Debug)]
    pub struct UnboundImage(pub MTLTexture);

    unsafe impl Send for UnboundImage {
    }
    unsafe impl Sync for UnboundImage {
    }
}

#[cfg(not(feature = "native_fence"))]
mod fence_related {
    use std::sync::{Arc, Mutex};
    #[derive(Debug)]
    pub struct Fence(pub Arc<Mutex<bool>>);
}

gfx_impl_resources!();

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
