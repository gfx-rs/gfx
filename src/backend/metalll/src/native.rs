use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::raw::{c_void, c_long, c_int};
use std::ptr;

use core;
use core::format;
use metal::*;
use objc;
use cocoa::foundation::NSUInteger;

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
pub struct Fence {}
#[derive(Debug)]
pub struct Mapping {}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct DescriptorHeap {}

#[derive(Debug)]
pub struct DescriptorSetPool {}
#[derive(Debug)]
pub struct DescriptorSet {}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct Heap {
    pub heap_type: core::HeapType,
    pub size: u64,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct PipelineLayout {}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct DescriptorSetLayout {}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct UnboundBuffer(pub MTLBuffer);

unsafe impl Send for UnboundBuffer {
}
unsafe impl Sync for UnboundBuffer {
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct UnboundImage(pub MTLTexture);

unsafe impl Send for UnboundImage {
}
unsafe impl Sync for UnboundImage {
}

#[derive(Debug)]
pub struct Buffer(pub MTLBuffer);

unsafe impl Send for Buffer {
}
unsafe impl Sync for Buffer {
}

#[repr(C)]
pub struct NSRange {
    pub location: NSUInteger,
    pub length: NSUInteger,
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
