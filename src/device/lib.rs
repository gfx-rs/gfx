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

#![crate_id = "github.com/bjz/gfx-rs#device:0.1"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(phase)]
#[phase(plugin, link)] extern crate log;
extern crate libc;

#[cfg(gl)] pub use self::gl::Device;
#[cfg(gl)] pub use dev = self::gl;
// #[cfg(d3d11)] ... // TODO

use std::kinds::marker;

pub mod shade;
pub mod target;
#[cfg(gl)] mod gl;


pub type VertexCount = u16;
pub type IndexCount = u16;
pub type AttributeSlot = u8;
pub type UniformBufferSlot = u8;
pub type TextureSlot = u8;

#[deriving(Show)]
pub enum BufferUsage {
    UsageStatic,
    UsageDynamic,
}


#[deriving(Show)]
pub enum Request {
    // Requests that require a reply:
    CallNewVertexBuffer(Vec<f32>),
    CallNewIndexBuffer(Vec<u16>),
    CallNewRawBuffer,
    CallNewArrayBuffer,
    CallNewShader(shade::Stage, Vec<u8>),
    CallNewProgram(Vec<dev::Shader>),
    CallNewFrameBuffer,
    // Requests that don't expect a reply:
    CastClear(target::ClearData),
    CastBindProgram(dev::Program),
    CastBindArrayBuffer(dev::ArrayBuffer),
    CastBindAttribute(AttributeSlot, dev::Buffer, u32, u32, u32),
    CastBindIndex(dev::Buffer),
    CastBindFrameBuffer(dev::FrameBuffer),
    CastBindTarget(target::Target, target::Plane),
    CastBindUniformBlock(dev::Program, u8, UniformBufferSlot, dev::Buffer),
    CastBindUniform(shade::Location, shade::UniformValue),
    //CastBindTexture(TextureSlot, dev::Texture, dev::Sampler),    //TODO
    CastUpdateBuffer(dev::Buffer, Vec<f32>),
    CastDraw(VertexCount, VertexCount),
    CastDrawIndexed(IndexCount, IndexCount),
    CastSwapBuffers,
}

#[deriving(Show)]
pub enum Reply {
    ReplyNewBuffer(dev::Buffer),
    ReplyNewArrayBuffer(dev::ArrayBuffer),
    ReplyNewShader(Result<dev::Shader, ()>),
    ReplyNewProgram(Result<shade::ProgramMeta, ()>),
    ReplyNewFrameBuffer(dev::FrameBuffer),
}

pub trait DeviceTask {
    // calls
    fn create_buffer(&mut self) -> dev::Buffer;
    fn create_array_buffer(&mut self) -> dev::ArrayBuffer;
    fn create_shader(&mut self, shade::Stage, code: &[u8]) -> Result<dev::Shader, ()>;
    fn create_program(&mut self, shaders: &[dev::Shader]) -> Result<shade::ProgramMeta, ()>;
    fn create_frame_buffer(&mut self) -> dev::FrameBuffer;
    // helpers
    fn update_buffer<T>(&mut self, dev::Buffer, data: &[T], BufferUsage);
    // casts
    fn process(&mut self, Request);
}

pub struct Server<P, D> {
    no_share: marker::NoShare,
    request_rx: Receiver<Request>,
    reply_tx: Sender<Reply>,
    graphics_context: P,
    device: D,
}

impl<Api, P: GraphicsContext<Api>, D: DeviceTask> Server<P, D> {
    pub fn make_current(&self) {
        self.graphics_context.make_current();
    }

    /// Update the platform. The client must manually update this on the main
    /// thread.
    pub fn update(&mut self) -> bool {
        // Get updates from the renderer and pass on results
        loop {
            match self.request_rx.recv_opt() {
                Ok(CastSwapBuffers) => {
                    self.graphics_context.swap_buffers();
                    break;
                },
                Ok(CallNewVertexBuffer(data)) => {
                    let name = self.device.create_buffer();
                    self.device.update_buffer(name, data.as_slice(), UsageStatic);
                    self.reply_tx.send(ReplyNewBuffer(name));
                },
                Ok(CallNewIndexBuffer(data)) => {
                    let name = self.device.create_buffer();
                    self.device.update_buffer(name, data.as_slice(), UsageStatic);
                    self.reply_tx.send(ReplyNewBuffer(name));
                },
                Ok(CallNewRawBuffer) => {
                    let name = self.device.create_buffer();
                    self.reply_tx.send(ReplyNewBuffer(name));
                },
                Ok(CallNewArrayBuffer) => {
                    let name = self.device.create_array_buffer();
                    self.reply_tx.send(ReplyNewArrayBuffer(name));
                },
                Ok(CallNewShader(stage, code)) => {
                    let name = self.device.create_shader(stage, code.as_slice());
                    self.reply_tx.send(ReplyNewShader(name));
                },
                Ok(CallNewProgram(code)) => {
                    let name = self.device.create_program(code.as_slice());
                    self.reply_tx.send(ReplyNewProgram(name));
                },
                Ok(CallNewFrameBuffer) => {
                    let name = self.device.create_frame_buffer();
                    self.reply_tx.send(ReplyNewFrameBuffer(name));
                },
                Ok(request) => self.device.process(request),
                Err(()) => return false,
            }
        }
        true
    }
}


pub trait GraphicsContext<Api> {
    fn swap_buffers(&self);
    fn make_current(&self);
}

pub trait GlProvider {
    fn get_proc_address(&self, &str) -> *const ::libc::c_void;
    fn is_extension_supported(&self, &str) -> bool;
}

#[deriving(Show)]
pub enum InitError {}

pub type Options<'a> = &'a GlProvider;

#[allow(visible_private_types)]
pub fn init<Api, P: GraphicsContext<Api>>(graphics_context: P, options: Options)
        -> Result<(Sender<Request>, Receiver<Reply>, Server<P, Device>), InitError> {
    let (request_tx, request_rx) = channel();
    let (reply_tx, reply_rx) = channel();

    let dev = Device::new(options);
    let server = Server {
        no_share: marker::NoShare,
        request_rx: request_rx,
        reply_tx: reply_tx,
        graphics_context: graphics_context,
        device: dev,
    };

    Ok((request_tx, reply_rx, server))
}
