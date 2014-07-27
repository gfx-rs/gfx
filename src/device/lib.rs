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

#![feature(phase, globs)]

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
pub mod rast;
pub mod shade;
pub mod target;
pub mod tex;
/* #[cfg(gl)] */ mod gl;

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

pub type VertexCount = u16;
pub type IndexCount = u16;
pub type AttributeSlot = u8;
pub type UniformBufferSlot = u8;
pub type TextureSlot = u8;

pub trait Blob {
    fn get_address(&self) -> uint;
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

#[deriving(Show)]
pub enum BufferUsage {
    UsageStatic,
    UsageDynamic,
    UsageStream,
}

#[deriving(Show)]
pub enum Request<T> {
    Call(T, CallRequest),
    Cast(CastRequest),
    SwapBuffers,
}

/// Requests that require a reply
#[deriving(Show)]
pub enum CallRequest {
    CreateBuffer(Option<Box<Blob + Send>>),
    CreateArrayBuffer,
    CreateShader(shade::Stage, shade::ShaderSource),
    CreateProgram(Vec<dev::Shader>),
    CreateTexture(tex::TextureInfo),
    CreateSampler(tex::SamplerInfo),
    CreateFrameBuffer,
}

/// Requests that don't expect a reply
#[deriving(Show)]
pub enum CastRequest {
    Clear(target::ClearData),
    BindProgram(dev::Program),
    BindArrayBuffer(dev::ArrayBuffer),
    BindAttribute(AttributeSlot, dev::Buffer, attrib::Count,
        attrib::Type, attrib::Stride, attrib::Offset),
    BindIndex(dev::Buffer),
    BindFrameBuffer(dev::FrameBuffer),
    /// Bind a `Plane` to a specific render target.
    BindTarget(target::Target, target::Plane),
    BindUniformBlock(dev::Program, u8, UniformBufferSlot, dev::Buffer),
    BindUniform(shade::Location, shade::UniformValue),
    BindTexture(TextureSlot, dev::Texture, dev::Sampler),
    SetPrimitiveState(rast::Primitive),
    SetDepthStencilState(Option<rast::Depth>, Option<rast::Stencil>, rast::CullMode),
    SetBlendState(Option<rast::Blend>),
    UpdateBuffer(dev::Buffer, Box<Blob + Send>),
    UpdateTexture(dev::Texture, tex::ImageInfo, Box<Blob + Send>),
    Draw(VertexCount, VertexCount),
    DrawIndexed(IndexCount, IndexCount),
}

#[deriving(Show)]
pub enum Reply<T> {
    ReplyNewBuffer(T, dev::Buffer),
    ReplyNewArrayBuffer(T, Result<dev::ArrayBuffer, ()>),
    ReplyNewShader(T, Result<dev::Shader, shade::CreateShaderError>),
    ReplyNewProgram(T, Result<shade::ProgramMeta, ()>),
    ReplyNewFrameBuffer(T, dev::FrameBuffer),
    ReplyNewTexture(T, dev::Texture),
    ReplyNewSampler(T, dev::Sampler),
}

/// An interface for performing draw calls using a specific graphics API
pub trait ApiBackEnd {
    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;
    // calls
    fn create_buffer(&mut self) -> dev::Buffer;
    fn create_array_buffer(&mut self) -> Result<dev::ArrayBuffer, ()>;
    fn create_shader(&mut self, shade::Stage, code: shade::ShaderSource) -> Result<dev::Shader, shade::CreateShaderError>;
    fn create_program(&mut self, shaders: &[dev::Shader]) -> Result<shade::ProgramMeta, ()>;
    fn create_frame_buffer(&mut self) -> dev::FrameBuffer;
    fn create_texture(&mut self, info: tex::TextureInfo) -> dev::Texture;
    fn create_sampler(&mut self, info: tex::SamplerInfo) -> dev::Sampler;
    /// Update the information stored in a specific buffer
    fn update_buffer(&mut self, dev::Buffer, data: &Blob, BufferUsage);
    /// Process a request from a `Device`
    fn process(&mut self, CastRequest);
}

pub struct Ack;

/// An API-agnostic device that manages incoming draw calls
pub struct Device<T, B, C> {
    no_share: marker::NoShare,
    request_rx: Receiver<Request<T>>,
    reply_tx: Sender<Reply<T>>,
    graphics_context: C,
    back_end: B,
    swap_ack: Sender<Ack>,
    close: comm::Close,
}

impl<T: Send, B: ApiBackEnd, C: GraphicsContext<B>> Device<T, B, C> {
    /// Signal to connected client that the device wants to close, and block
    /// until it has disconnected.
    pub fn close(&self) {
        self.close.now()
    }

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
            CreateTexture(info) => {
                let name = self.back_end.create_texture(info);
                ReplyNewTexture(token, name)
            },
            CreateSampler(info) => {
                let name = self.back_end.create_sampler(info);
                ReplyNewSampler(token, name)
            },
            CreateProgram(code) => {
                let name = self.back_end.create_program(code.as_slice());
                ReplyNewProgram(token, name)
            },
            CreateFrameBuffer => {
                let name = self.back_end.create_frame_buffer();
                ReplyNewFrameBuffer(token, name)
            },
        }
    }

    /// Update the platform. The client must manually update this on the main
    /// thread.
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
                    self.swap_ack.send(Ack);
                    break;
                },
                Err(()) => return,
            }
        }
    }
}

pub trait GraphicsContext<T> {
    fn swap_buffers(&self);
    fn make_current(&self);
}

pub trait GlProvider {
    fn get_proc_address(&self, &str) -> *const ::libc::c_void;
}

#[deriving(Show)]
pub enum InitError {}

pub type QueueSize = u8;

// TODO: Generalise for different back-ends
#[allow(visible_private_types)]
pub fn init<T: Send, C: GraphicsContext<GlBackEnd>, P: GlProvider>(graphics_context: C, provider: P, queue_size: QueueSize)
        -> Result<(Sender<Request<T>>, Receiver<Reply<T>>, Device<T, GlBackEnd, C>, Receiver<Ack>, comm::ShouldClose), InitError> {
    let (request_tx, request_rx) = channel();
    let (reply_tx, reply_rx) = channel();
    let (swap_tx, swap_rx) = channel();
    let (close, should_close) = comm::close_stream();

    for _ in range(0, queue_size) {
        swap_tx.send(Ack);
    }

    let gl = GlBackEnd::new(&provider);
    let device = Device {
        no_share: marker::NoShare,
        request_rx: request_rx,
        reply_tx: reply_tx,
        graphics_context: graphics_context,
        back_end: gl,
        swap_ack: swap_tx,
        close: close,
    };

    Ok((request_tx, reply_rx, device, swap_rx, should_close))
}
