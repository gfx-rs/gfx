use core::memory::{Bind, Usage};
use std::os::raw::{c_void, c_long, c_int};
use std::sync::{Arc, Mutex};
use metal::*;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawBuffer(pub *mut MTLBuffer);
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawTexture(pub *mut MTLTexture);
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Sampler(pub MTLSamplerState);
unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Rtv(pub *mut MTLTexture);
unsafe impl Send for Rtv {}
unsafe impl Sync for Rtv {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Dsv(pub *mut MTLTexture, pub Option<u16>);
unsafe impl Send for Dsv {}
unsafe impl Sync for Dsv {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Srv(pub *mut MTLTexture);
unsafe impl Send for Srv {}
unsafe impl Sync for Srv {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Uav;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Shader {
    pub(crate) func: MTLFunction,
}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    pub(crate) vs: MTLFunction,
    pub(crate) ps: MTLFunction,
}
unsafe impl Send for Program {}
unsafe impl Sync for Program {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Pipeline {
    pub(crate) pipeline: MTLRenderPipelineState,
    pub(crate) depth_stencil: Option<MTLDepthStencilState>,
    pub(crate) winding: MTLWinding,
    pub(crate) cull: MTLCullMode,
    pub(crate) fill: MTLTriangleFillMode,
    pub(crate) alpha_to_one: bool,
    pub(crate) alpha_to_coverage: bool,
    pub(crate) depth_bias: i32,
    pub(crate) slope_scaled_depth_bias: i32,
    pub(crate) depth_clip: bool,
}
unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

pub struct ShaderLibrary {
    pub(crate) lib: MTLLibrary,
}
unsafe impl Send for ShaderLibrary {}
unsafe impl Sync for ShaderLibrary {}

// ShaderLibrary isn't handled via Device.cleanup(). Not really an issue since it will usually
// live for the entire application lifetime and be cloned rarely.
impl Drop for ShaderLibrary {
    fn drop(&mut self) {
        unsafe { self.lib.release() };
    }
}

impl Clone for ShaderLibrary {
    fn clone(&self) -> Self {
        unsafe { self.lib.retain() };
        ShaderLibrary { lib: self.lib }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Buffer(pub RawBuffer, pub Usage, pub Bind);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Texture(pub RawTexture, pub Usage);

#[derive(Clone, Debug)]
pub struct Fence(pub Arc<Mutex<bool>>);

#[derive(Debug)]
pub struct Semaphore(pub *mut c_void);
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

pub const kCVPixelFormatType_32RGBA: u32 = (b'R' as u32) << 24 | (b'G' as u32) << 16 | (b'B' as u32) << 8 | b'A' as u32;

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
