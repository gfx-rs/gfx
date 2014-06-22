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

use render::mesh::Mesh;
use Platform;

#[cfg(gl)] mod gl;

pub type Color = [f32, ..4];


pub enum Request {
    // Requests that require a reply:
    CallNewBuffer(Vec<f32>),
    CallNewArrayBuffer,
    CallNewShader(char, Vec<u8>),
    CallNewProgram(Vec<dev::Shader>),
    // Requests that don't expect a reply:
    CastClear(Color),
    CastBindProgram(dev::Program),
    CastDraw(Mesh),
    CastSwapBuffers,
}

pub enum Reply {
    ReplyNewBuffer(dev::Buffer),
    ReplyNewArrayBuffer(dev::ArrayBuffer),
    ReplyNewShader(dev::Shader),
    ReplyNewProgram(dev::Program),
}

pub struct Client {
    stream: DuplexStream<Request, Reply>,
}

impl Client {
    pub fn clear(&self, r: f32, g: f32, b: f32) {
        let color = [r, g, b, 1.0];
        self.stream.send(CastClear(color));
    }

    pub fn bind_program(&self, prog: dev::Program) {
        self.stream.send(CastBindProgram(prog));
    }

    pub fn draw(&self, mesh: Mesh) {
        self.stream.send(CastDraw(mesh));
    }

    pub fn end_frame(&self) {
        self.stream.send(CastSwapBuffers);
    }

    pub fn new_shader(&self, kind: char, code: Vec<u8>) -> dev::Shader {
        self.stream.send(CallNewShader(kind, code));
        match self.stream.recv() {
            ReplyNewShader(name) => name,
            _ => fail!("unexpected device reply")
        }
    }

    pub fn new_program(&self, shaders: Vec<dev::Shader>) -> dev::Program {
        self.stream.send(CallNewProgram(shaders));
        match self.stream.recv() {
            ReplyNewProgram(name) => name,
            _ => fail!("unexpected device reply")
        }
    }

    pub fn new_buffer(&self, data: Vec<f32>) -> dev::Buffer {
        self.stream.send(CallNewBuffer(data));
        match self.stream.recv() {
            ReplyNewBuffer(name) => name,
            _ => fail!("unexpected device reply")
        }
    }

    pub fn new_array_buffer(&self) -> dev::ArrayBuffer {
        self.stream.send(CallNewArrayBuffer);
        match self.stream.recv() {
            ReplyNewArrayBuffer(name) => name,
            _ => fail!("unexpected device reply")
        }
    }
}

pub struct Server<P> {
    no_send: marker::NoSend,
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
    platform: P,
    device: Device,
}

impl<Api, P: Platform<Api>> Server<P> {
    /// Update the platform. The client must manually update this on the main
    /// thread.
    pub fn update(&mut self) -> bool {
        // Get updates from the renderer and pass on results
        loop {
            match self.stream.try_recv() {
                Ok(CastClear(color)) => {
                    self.device.clear(color.as_slice());
                },
                Ok(CastBindProgram(prog)) => {
                    self.device.bind_program(prog);
                },
                Ok(CastDraw(mesh)) => {
                    self.device.bind_array_buffer(mesh.array_buffer);
                    self.device.bind_vertex_buffer(mesh.vertex_buf);
                    self.device.bind_attribute(0, mesh.num_vertices, 8);
                    self.device.draw(0, mesh.num_vertices);
                },
                Ok(CastSwapBuffers) => {
                    break;
                },
                Ok(CallNewBuffer(data)) => {
                    let name = self.device.create_buffer(data.as_slice());
                    self.stream.send(ReplyNewBuffer(name));
                },
                Ok(CallNewArrayBuffer) => {
                    let name = self.device.create_array_buffer();
                    self.stream.send(ReplyNewArrayBuffer(name));
                },
                Ok(CallNewShader(kind, code)) => {
                    let name = self.device.create_shader(kind, code.as_slice());
                    self.stream.send(ReplyNewShader(name));
                },
                Ok(CallNewProgram(code)) => {
                    let name = self.device.create_program(code.as_slice());
                    self.stream.send(ReplyNewProgram(name));
                },
                Err(comm::Empty) => break,
                Err(comm::Disconnected) => return false,
            }
        }
        self.platform.swap_buffers();
        true
    }
}

#[deriving(Show)]
pub enum InitError {}

pub fn init<Api, P: Platform<Api>>(platform: P, options: super::Options)
        -> Result<(Client, Server<P>), InitError> {
    let (client_stream, server_stream) = comm::duplex();

    let client = Client {
        stream: client_stream,
    };
    let dev = Device::new(options);
    let server = Server {
        no_send: marker::NoSend,
        no_share: marker::NoShare,
        stream: server_stream,
        platform: platform,
        device: dev,
    };

    Ok((client, server))
}
