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
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(phase)]

#[phase(plugin, link)] extern crate log;
extern crate libc;
extern crate comm;

#[cfg(gl)] pub use gl::GlBackEnd;
#[cfg(gl)] pub use dev = self::gl;
// #[cfg(d3d11)] ... // TODO

use std::kinds::marker;

pub mod attrib;
pub mod rast;
pub mod shade;
pub mod target;
#[cfg(gl)] mod gl;


#[deriving(Show)]
pub struct Capabilities {
    shader_model: shade::ShaderModel,
    max_draw_buffers : uint,
    max_texture_size : uint,
    max_vertex_attributes: uint,
    uniform_block_supported: bool,
    array_buffer_supported: bool,
}

pub type VertexCount = u16;
pub type IndexCount = u16;
pub type AttributeSlot = u8;
pub type UniformBufferSlot = u8;
pub type TextureSlot = u8;

#[deriving(Show)]
pub enum BufferUsage {
    UsageStatic,
    UsageDynamic,
    UsageStream,
}

#[deriving(Show)]
pub enum Request<T> {
    // Requests that require a reply:
    CallAck(T),
    CallNewVertexBuffer(T, Vec<f32>),
    CallNewIndexBuffer(T, Vec<u16>),
    CallNewRawBuffer(T),
    CallNewArrayBuffer(T),
    CallNewShader(T, shade::Stage, shade::ShaderSource),
    CallNewProgram(T, Vec<dev::Shader>),
    CallNewFrameBuffer(T),
    // Requests that don't expect a reply:
    CastClear(target::ClearData),
    CastBindProgram(dev::Program),
    CastBindArrayBuffer(dev::ArrayBuffer),
    CastBindAttribute(AttributeSlot, dev::Buffer, attrib::Count,
        attrib::Type, attrib::Stride, attrib::Offset),
    CastBindIndex(dev::Buffer),
    CastBindFrameBuffer(dev::FrameBuffer),
    CastBindTarget(target::Target, target::Plane),
    CastBindUniformBlock(dev::Program, u8, UniformBufferSlot, dev::Buffer),
    CastBindUniform(shade::Location, shade::UniformValue),
    //CastBindTexture(TextureSlot, dev::Texture, dev::Sampler),    //TODO
    CastPrimitiveState(rast::Primitive),
    CastDepthStencilState(Option<rast::Depth>, Option<rast::Stencil>, rast::CullMode),
    CastBlendState(Option<rast::Blend>),
    CastUpdateBuffer(dev::Buffer, Vec<f32>),
    CastDraw(VertexCount, VertexCount),
    CastDrawIndexed(IndexCount, IndexCount),
    CastSwapBuffers,
}

#[deriving(Show)]
pub enum Reply<T> {
    ReplyAck(T),
    ReplyNewBuffer(T, dev::Buffer),
    ReplyNewArrayBuffer(T, Result<dev::ArrayBuffer, ()>),
    ReplyNewShader(T, Result<dev::Shader, shade::CreateShaderError>),
    ReplyNewProgram(T, Result<shade::ProgramMeta, ()>),
    ReplyNewFrameBuffer(T, dev::FrameBuffer),
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
    /// Update the information stored in a specific buffer
    fn update_buffer<T>(&mut self, dev::Buffer, data: &[T], BufferUsage);
    /// Process a request from a `Device`
    fn process(&mut self, Request);
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

impl<T, B: ApiBackEnd, C: GraphicsContext<B>> Device<B, C> {
    /// Signal to connected client that the device wants to close, and block
    /// until it has disconnected.
    pub fn close(&self) {
        self.close.now()
    }

    pub fn make_current(&self) {
        self.graphics_context.make_current();
    }

    /// Update the platform. The client must manually update this on the main
    /// thread.
    pub fn update(&mut self) {
        // Get updates from the renderer and pass on results
        loop {
            match self.request_rx.recv_opt() {
                Ok(CallAck(token)) => self.reply_tx.send(ReplyAck(token)),
                Ok(CastSwapBuffers) => {
                    self.graphics_context.swap_buffers();
                    self.swap_ack.send(Ack);
                    break;
                },
                Ok(CallNewVertexBuffer(token, data)) => {
                    let name = self.back_end.create_buffer();
                    self.back_end.update_buffer(name, data.as_slice(), UsageStatic);
                    self.reply_tx.send(ReplyNewBuffer(token, name));
                },
                Ok(CallNewIndexBuffer(token, data)) => {
                    let name = self.back_end.create_buffer();
                    self.back_end.update_buffer(name, data.as_slice(), UsageStatic);
                    self.reply_tx.send(ReplyNewBuffer(token, name));
                },
                Ok(CallNewRawBuffer(token)) => {
                    let name = self.back_end.create_buffer();
                    self.reply_tx.send(ReplyNewBuffer(token, name));
                },
                Ok(CallNewArrayBuffer(token)) => {
                    let name = self.back_end.create_array_buffer();
                    self.reply_tx.send(ReplyNewArrayBuffer(token, name));
                },
                Ok(CallNewShader(token, stage, code)) => {
                    let name = self.back_end.create_shader(stage, code);
                    self.reply_tx.send(ReplyNewShader(token, name));
                },
                Ok(CallNewProgram(token, code)) => {
                    let name = self.back_end.create_program(code.as_slice());
                    self.reply_tx.send(ReplyNewProgram(token, name));
                },
                Ok(CallNewFrameBuffer(token)) => {
                    let name = self.back_end.create_frame_buffer();
                    self.reply_tx.send(ReplyNewFrameBuffer(token, name));
                },
                Ok(request) => self.back_end.process(request),
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
    fn is_extension_supported(&self, &str) -> bool;
}

#[deriving(Show)]
pub enum InitError {}

pub type QueueSize = u8;

// TODO: Generalise for different back-ends
#[allow(visible_private_types)]
pub fn init<C: GraphicsContext<GlBackEnd>, P: GlProvider>(graphics_context: C, provider: P, queue_size: QueueSize)
        -> Result<(Sender<Request>, Receiver<Reply>, Device<GlBackEnd, C>, Receiver<Ack>, comm::ShouldClose), InitError> {
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
