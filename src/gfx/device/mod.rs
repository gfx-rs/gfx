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

#[cfg(gl)] pub use self::gl::Device;
#[cfg(gl)] pub use dev = self::gl;
// #[cfg(d3d11)] ... // TODO

use std::comm;
use std::comm::DuplexStream;
use std::kinds::marker;

use GraphicsContext;

pub mod shade;
#[cfg(gl)] mod gl;

pub type Color = [f32, ..4];
pub type VertexCount = u16;
pub type IndexCount = u16;
pub type AttributeSlot = u8;
pub type UniformBufferSlot = u8;
pub type TextureSlot = u8;

pub enum BufferUsage {
    UsageStatic,
    UsageDynamic,
}


pub enum Request {
    // Requests that require a reply:
    CallNewVertexBuffer(Vec<f32>),
    CallNewIndexBuffer(Vec<u16>),
    CallNewRawBuffer,
    CallNewArrayBuffer,
    CallNewShader(shade::Stage, Vec<u8>),
    CallNewProgram(Vec<dev::Shader>),
    // Requests that don't expect a reply:
    CastClear(Color),
    CastBindProgram(dev::Program),
    CastBindArrayBuffer(dev::ArrayBuffer),
    CastBindAttribute(AttributeSlot, dev::Buffer, u32, u32, u32),
    CastBindIndex(dev::Buffer),
    CastBindFrameBuffer(dev::FrameBuffer),
    CastBindUniformBlock(dev::Program, u8, UniformBufferSlot, dev::Buffer),
    CastBindUniform(shade::Location, shade::UniformValue),
    //CastBindTexture(TextureSlot, dev::Texture, dev::Sampler),    //TODO
    CastUpdateBuffer(dev::Buffer, Vec<f32>),
    CastDraw(VertexCount, VertexCount),
    CastDrawIndexed(IndexCount, IndexCount),
    CastSwapBuffers,
}

pub enum Reply {
    ReplyNewBuffer(dev::Buffer),
    ReplyNewArrayBuffer(dev::ArrayBuffer),
    ReplyNewShader(Result<dev::Shader, ()>),
    ReplyNewProgram(Result<shade::ProgramMeta, ()>),
}


pub type Client = DuplexStream<Request, Reply>;

pub struct Server<P> {
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
    graphics_context: P,
    device: Device,
}

impl<Api, P: GraphicsContext<Api>> Server<P> {
    pub fn make_current(&self) {
        self.graphics_context.make_current();
    }

    /// Update the platform. The client must manually update this on the main
    /// thread.
    pub fn update(&mut self) -> bool {
        // Get updates from the renderer and pass on results
        loop {
            match self.stream.recv_opt() {
                Ok(CastClear(color)) => {
                    self.device.clear(color.as_slice());
                },
                Ok(CastBindProgram(prog)) => {
                    self.device.bind_program(prog);
                },
                Ok(CastBindArrayBuffer(abuf)) => {
                    self.device.bind_array_buffer(abuf);
                },
                Ok(CastBindAttribute(index, buf, count, offset, stride)) => {
                    self.device.bind_vertex_buffer(buf);
                    self.device.bind_attribute(index, count as u32, offset, stride);
                },
                Ok(CastBindIndex(buf)) => {
                    self.device.bind_index_buffer(buf);
                },
                Ok(CastBindFrameBuffer(fbo)) => {
                    self.device.bind_frame_buffer(fbo);
                },
                Ok(CastBindUniformBlock(prog, index, loc, buf)) => {
                    self.device.bind_uniform_block(prog, index, loc);
                    self.device.map_uniform_buffer(loc, buf);
                },
                Ok(CastBindUniform(loc, value)) => {
                    self.device.bind_uniform(loc, value);
                },
                Ok(CastUpdateBuffer(buf, data)) => {
                    self.device.update_buffer(buf, data.as_slice(), UsageDynamic);
                },
                Ok(CastDraw(offset, count)) => {
                    self.device.draw(offset as u32, count as u32);
                },
                Ok(CastDrawIndexed(offset, count)) => {
                    self.device.draw_index(offset, count);
                },
                Ok(CastSwapBuffers) => {
                    self.graphics_context.swap_buffers();
                    break;
                },
                Ok(CallNewVertexBuffer(data)) => {
                    let name = self.device.create_buffer();
                    self.device.update_buffer(name, data.as_slice(), UsageStatic);
                    self.stream.send(ReplyNewBuffer(name));
                },
                Ok(CallNewIndexBuffer(data)) => {
                    let name = self.device.create_buffer();
                    self.device.update_buffer(name, data.as_slice(), UsageStatic);
                    self.stream.send(ReplyNewBuffer(name));
                },
                Ok(CallNewRawBuffer) => {
                    let name = self.device.create_buffer();
                    self.stream.send(ReplyNewBuffer(name));
                },
                Ok(CallNewArrayBuffer) => {
                    let name = self.device.create_array_buffer();
                    self.stream.send(ReplyNewArrayBuffer(name));
                },
                Ok(CallNewShader(stage, code)) => {
                    let name = self.device.create_shader(stage, code.as_slice());
                    self.stream.send(ReplyNewShader(name));
                },
                Ok(CallNewProgram(code)) => {
                    let name = self.device.create_program(code.as_slice());
                    self.stream.send(ReplyNewProgram(name));
                },
                Err(()) => return false,
            }
        }
        true
    }
}

#[deriving(Show)]
pub enum InitError {}

pub fn init<Api, P: GraphicsContext<Api>>(graphics_context: P, options: super::Options)
        -> Result<(Client, Server<P>), InitError> {
    let (client_stream, server_stream) = comm::duplex();

    let dev = Device::new(options);
    let server = Server {
        no_share: marker::NoShare,
        stream: server_stream,
        graphics_context: graphics_context,
        device: dev,
    };

    Ok((client_stream, server))
}
