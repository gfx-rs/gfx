// Copyright 2014 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![crate_name = "device"]
#![comment = "Back-ends to abstract over the differences between low-level, \
              platform-specific graphics APIs"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(phase)]
#![deny(missing_doc)]

//! Graphics device. Not meant for direct use.

#[phase(plugin, link)] extern crate log;
extern crate libc;
extern crate comm;

// when cargo is ready, re-enable the cfg's
/* #[cfg(gl)] */ pub use gl::GlBackEnd;
/* #[cfg(gl)] */ pub use dev = self::gl;
// #[cfg(d3d11)] ... // TODO

use std::fmt;
use std::kinds::marker;
use std::mem::size_of;

pub mod attrib;
pub mod shade;
pub mod state;
pub mod target;
pub mod tex;
/* #[cfg(gl)] */ mod gl;

/// Features that the device supports.
#[deriving(Show)]
pub struct Capabilities {
    shader_model: shade::ShaderModel,
    max_draw_buffers : uint,
    max_texture_size : uint,
    max_vertex_attributes: uint,
    uniform_block_supported: bool,
    array_buffer_supported: bool,
    sampler_objects_supported: bool,
    immutable_storage_supported: bool,
}

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw index count.
pub type IndexCount = u32;
/// Index of a uniform block.
pub type UniformBlockIndex = u8;
/// Slot for an attribute.
pub type AttributeSlot = u8;
/// Slot for a uniform buffer object.
pub type UniformBufferSlot = u8;
/// Slot a texture can be bound to.
pub type TextureSlot = u8;

/// A trait that slice-like types implement.
pub trait Blob {
    /// Get the address to the data this `Blob` stores.
    fn get_address(&self) -> uint;
    /// Get the number of bytes in this blob.
    fn get_size(&self) -> uint;
}

impl<T: Send> Blob for Vec<T> {
    fn get_address(&self) -> uint {
        self.as_ptr() as uint
    }
    fn get_size(&self) -> uint {
        self.len() * size_of::<T>()
    }
}

impl fmt::Show for Box<Blob + Send> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blob({:#x}, {})", self.get_address(), self.get_size())
    }
}

/// A hint as to how this buffer will be used.
///
/// The nature of these hints make them very implementation specific. Different drivers on
/// different hardware will handle them differently. Only careful profiling will tell which is the
/// best to use for a specific buffer.
#[deriving(Clone, PartialEq, Show)]
#[repr(u8)]
pub enum BufferUsage {
    /// Once uploaded, this buffer will rarely change, but will be read from often.
    UsageStatic,
    /// This buffer will be updated "frequently", and will be read from multiple times between
    /// updates.
    UsageDynamic,
    /// This buffer always or almost always be updated after each read.
    UsageStream,
}

/// Surface creation/update error.
#[deriving(Clone, PartialEq, Show)]
pub enum SurfaceError {
    /// Failed to map a given format to the device
    UnsupportedSurfaceFormat,
}

/// Texture creation/update error.
#[deriving(Clone, PartialEq, Show)]
pub enum TextureError {
    /// Failed to map a given format to the device
    UnsupportedTextureFormat,
}

/// Device request.
#[deriving(Show)]
pub enum Request<T> {
    /// A request that requires a reply - has the device creating something.
    Call(T, CallRequest),
    /// A request that does not require a reply - has the device modifying something.
    Cast(CastRequest),
    /// Swap the front and back buffers, displaying what has been drawn so far. Indicates the end
    /// of a frame.
    SwapBuffers,
}

/// Requests that require a reply
#[allow(missing_doc)]
#[deriving(Show)]
pub enum CallRequest {
    CreateBuffer(Option<Box<Blob + Send>>),
    CreateArrayBuffer,
    CreateShader(shade::Stage, shade::ShaderSource),
    CreateProgram(Vec<dev::Shader>),
    CreateFrameBuffer,
    CreateSurface(tex::SurfaceInfo),
    CreateTexture(tex::TextureInfo),
    CreateSampler(tex::SamplerInfo),
}

/// Requests that don't expect a reply
#[allow(missing_doc)]
#[deriving(Show)]
pub enum CastRequest {
    Clear(target::ClearData),
    BindProgram(dev::Program),
    BindArrayBuffer(dev::ArrayBuffer),
    BindAttribute(AttributeSlot, dev::Buffer, attrib::Count,
        attrib::Type, attrib::Stride, attrib::Offset),
    BindIndex(dev::Buffer),
    BindFrameBuffer(dev::FrameBuffer),
    /// Unbind any surface from the specified target slot
    UnbindTarget(target::Target),
    /// Bind a surface to the specified target slot
    BindTargetSurface(target::Target, dev::Surface),
    /// Bind a level of the texture to the specified target slot
    BindTargetTexture(target::Target, dev::Texture, target::Level, Option<target::Layer>),
    BindUniformBlock(dev::Program, UniformBufferSlot, UniformBlockIndex, dev::Buffer),
    BindUniform(shade::Location, shade::UniformValue),
    BindTexture(TextureSlot, tex::TextureKind, dev::Texture, Option<(dev::Sampler, tex::SamplerInfo)>),
    SetPrimitiveState(state::Primitive),
    SetViewport(target::Rect),
    SetScissor(Option<target::Rect>),
    SetDepthStencilState(Option<state::Depth>, Option<state::Stencil>, state::CullMode),
    SetBlendState(Option<state::Blend>),
    SetColorMask(state::ColorMask),
    UpdateBuffer(dev::Buffer, Box<Blob + Send>),
    UpdateTexture(tex::TextureKind, dev::Texture, tex::ImageInfo, Box<Blob + Send>),
    Draw(VertexCount, VertexCount),
    DrawIndexed(IndexCount, IndexCount),
    /// Resource deletion
    DeleteBuffer(dev::Buffer),
    DeleteShader(dev::Shader),
    DeleteProgram(dev::Program),
    DeleteSurface(dev::Surface),
    DeleteTexture(dev::Texture),
    DeleteSampler(dev::Sampler),
}

/// Reply to a `Call`
#[allow(missing_doc)]
#[deriving(Show)]
pub enum Reply<T> {
    ReplyNewBuffer(T, dev::Buffer),
    ReplyNewArrayBuffer(T, Result<dev::ArrayBuffer, ()>),
    ReplyNewShader(T, Result<dev::Shader, shade::CreateShaderError>),
    ReplyNewProgram(T, Result<shade::ProgramMeta, ()>),
    ReplyNewFrameBuffer(T, dev::FrameBuffer),
    ReplyNewSurface(T, dev::Surface),
    ReplyNewTexture(T, dev::Texture),
    ReplyNewSampler(T, dev::Sampler),
}

/// An interface for performing draw calls using a specific graphics API
#[allow(missing_doc)]
pub trait ApiBackEnd {
    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;
    // calls
    fn create_buffer(&mut self) -> dev::Buffer;
    fn create_array_buffer(&mut self) -> Result<dev::ArrayBuffer, ()>;
    fn create_shader(&mut self, shade::Stage, code: shade::ShaderSource) -> Result<dev::Shader, shade::CreateShaderError>;
    fn create_program(&mut self, shaders: &[dev::Shader]) -> Result<shade::ProgramMeta, ()>;
    fn create_frame_buffer(&mut self) -> dev::FrameBuffer;
    fn create_surface(&mut self, info: tex::SurfaceInfo) -> Result<dev::Surface, SurfaceError>;
    fn create_texture(&mut self, info: tex::TextureInfo) -> Result<dev::Texture, TextureError>;
    fn create_sampler(&mut self, info: tex::SamplerInfo) -> dev::Sampler;
    /// Update the information stored in a specific buffer
    fn update_buffer(&mut self, dev::Buffer, data: &Blob, BufferUsage);
    /// Process a request from a `Device`
    fn process(&mut self, CastRequest);
}

/// Token used for buffer swap acknowledgement.
pub struct Ack;

/// An API-agnostic device that manages incoming draw calls
pub struct Device<T, B, C> {
    no_share: marker::NoShare,
    request_rx: Receiver<Request<T>>,
    reply_tx: Sender<Reply<T>>,
    graphics_context: C,
    back_end: B,
    frame_done: Receiver<Ack>,
    close: comm::Close,
}

impl<T: Send, B: ApiBackEnd, C: GraphicsContext<B>> Device<T, B, C> {
    /// Signal to connected client that the device wants to close, and block
    /// until it has disconnected.
    pub fn close(&self) {
        self.close.now()
    }

    /// Make this device's context current for the thread.
    ///
    /// This is a GLism that might be removed, especially as multithreading support evolves.
    pub fn make_current(&self) {
        self.graphics_context.make_current();
    }

    /// Process a call request, return a single reply for it
    fn process(&mut self, token: T, call: CallRequest) -> Reply<T> {
        match call {
            CreateBuffer(data) => {
                let name = self.back_end.create_buffer();
                match data {
                    Some(blob) => self.back_end.update_buffer(name, blob, UsageStatic),
                    None => (),
                }
                ReplyNewBuffer(token, name)
            },
            CreateArrayBuffer => {
                let name = self.back_end.create_array_buffer();
                ReplyNewArrayBuffer(token, name)
            },
            CreateShader(stage, code) => {
                let name = self.back_end.create_shader(stage, code);
                ReplyNewShader(token, name)
            },
            CreateProgram(code) => {
                let name = self.back_end.create_program(code.as_slice());
                ReplyNewProgram(token, name)
            },
            CreateFrameBuffer => {
                let name = self.back_end.create_frame_buffer();
                ReplyNewFrameBuffer(token, name)
            },
            CreateSurface(info) => {
                match self.back_end.create_surface(info) {
                    Ok(name) => ReplyNewSurface(token, name),
                    Err(_e) => unimplemented!(),
                }
            },
            CreateTexture(info) => {
                match self.back_end.create_texture(info) {
                    Ok(name) => ReplyNewTexture(token, name),
                    Err(_e) => unimplemented!(),
                }
            },
            CreateSampler(info) => {
                let name = self.back_end.create_sampler(info);
                ReplyNewSampler(token, name)
            },
        }
    }

    /// Process all requests received, including requests received while this method is executing.
    /// The client must manually call this on the main thread, or else the renderer will have no
    /// effect.
    pub fn update(&mut self) {
        // Get updates from the renderer and pass on results
        loop {
            match self.request_rx.recv_opt() {
                Ok(Call(token, call)) => {
                    let reply = self.process(token, call);
                    self.reply_tx.send(reply);
                },
                Ok(Cast(cast)) => self.back_end.process(cast),
                Ok(SwapBuffers) => {
                    self.graphics_context.swap_buffers();
                    self.frame_done.recv();
                    break;
                },
                Err(()) => return,
            }
        }
    }
}

/// A trait that OpenGL contexts implement.
pub trait GraphicsContext<T> {
    /// Swap the front and back buffers, displaying what has been rendered.
    fn swap_buffers(&self);
    /// Make this context active on the calling thread.
    fn make_current(&self);
}

/// A trait that OpenGL interfaces implement.
pub trait GlProvider {
    /// Load the GL command with the given name.
    fn get_proc_address(&self, function_name: &str) -> *const ::libc::c_void;
}

/// An error type for device initialization.
#[deriving(Show)]
pub enum InitError {}

/// Type representing the number of frames to queue before the renderer blocks.
pub type QueueSize = u8;

// TODO: Generalise for different back-ends
/// Initialize a device for a given context and provider pair, with a given queue size.
pub fn init<T: Send, C: GraphicsContext<GlBackEnd>, P: GlProvider>(graphics_context: C, provider: P, queue_size: QueueSize)
        -> Result<(Sender<Request<T>>, Receiver<Reply<T>>, Device<T, GlBackEnd, C>, SyncSender<Ack>, comm::ShouldClose), InitError> {
    let (request_tx, request_rx) = channel();
    let (reply_tx, reply_rx) = channel();
    let (swap_tx, swap_rx) = sync_channel(queue_size as uint);
    let (close, should_close) = comm::close_stream();

    let gl = GlBackEnd::new(&provider);
    let device = Device {
        no_share: marker::NoShare,
        request_rx: request_rx,
        reply_tx: reply_tx,
        graphics_context: graphics_context,
        back_end: gl,
        frame_done: swap_rx,
        close: close,
    };

    Ok((request_tx, reply_rx, device, swap_tx, should_close))
}
