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

use server;
use Platform;

#[cfg(gl)] mod gl;

pub type Color = [f32, ..4];

pub struct Mesh {
    pub num_vertices: u32,
    pub vertex_buf: dev::Buffer,
    pub array_buffer: dev::ArrayBuffer,
}


pub enum Call {
    CallNewBuffer(Vec<f32>),
    CallNewArrayBuffer,
    CallNewShader(char, Vec<u8>),
    CallNewProgram(Vec<dev::Shader>),
}

pub enum Cast {
    CastClear(Color),
    CastBindProgram(dev::Program),
    CastDraw(Mesh),
    CastSwapBuffers,
}

pub type Request = server::Request<Call, Cast>;

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
    fn call(&self, msg: Call) -> Reply {
        self.stream.send(server::Call(msg));
        //TODO: make it asynchronous, we need to give it time
        // to process requests before demanding the results
        self.stream.recv()
    }

    fn cast(&self, msg: Cast) {
        self.stream.send(server::Cast(msg));
    }

    pub fn clear(&self, r: f32, g: f32, b: f32) {
        let color = [r, g, b, 1.0];
        self.cast(CastClear(color));
    }

    pub fn bind_program(&self, prog: dev::Program) {
        self.cast(CastBindProgram(prog));
    }

    pub fn draw(&self, mesh: Mesh) {
        self.cast(CastDraw(mesh));
    }

    pub fn end_frame(&self) {
        self.cast(CastSwapBuffers);
    }

    pub fn new_shader(&self, kind: char, code: Vec<u8>) -> dev::Shader {
        match self.call(CallNewShader(kind, code)) {
            ReplyNewShader(name) => name,
            _ => fail!("unexpected device reply")
        }
    }

    pub fn new_program(&self, shaders: Vec<dev::Shader>) -> dev::Program {
        match self.call(CallNewProgram(shaders)) {
            ReplyNewProgram(name) => name,
            _ => fail!("unexpected device reply")
        }
    }

    pub fn new_buffer(&self, data: Vec<f32>) -> dev::Buffer {
        match self.call(CallNewBuffer(data)) {
            ReplyNewBuffer(name) => name,
            _ => fail!("unexpected device reply")
        }
    }

    pub fn new_array_buffer(&self) -> dev::ArrayBuffer {
        match self.call(CallNewArrayBuffer) {
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
    pub fn update(&mut self) {
        // Get updates from the renderer and pass on results
        'recv: loop {
            match self.stream.try_recv() {
                Ok(server::Cast(CastClear(color))) => {
                    self.device.clear(color.as_slice());
                },
                Ok(server::Cast(CastBindProgram(prog))) => {
                    self.device.bind_program(prog);
                },
                Ok(server::Cast(CastDraw(mesh))) => {
                    self.device.bind_array_buffer(mesh.array_buffer);
                    self.device.bind_vertex_buffer(mesh.vertex_buf);
                    self.device.bind_attribute(0, mesh.num_vertices, 8);
                    self.device.draw(0, mesh.num_vertices);
                },
                Ok(server::Cast(CastSwapBuffers)) => {
                    break 'recv
                },
                Ok(server::Call(CallNewBuffer(data))) => {
                    let name = self.device.create_buffer(data.as_slice());
                    self.stream.send(ReplyNewBuffer(name));
                },
                Ok(server::Call(CallNewArrayBuffer)) => {
                    let name = self.device.create_array_buffer();
                    self.stream.send(ReplyNewArrayBuffer(name));
                },
                Ok(server::Call(CallNewShader(kind, code))) => {
                    let name = self.device.create_shader(kind, code.as_slice());
                    self.stream.send(ReplyNewShader(name));
                },
                Ok(server::Call(CallNewProgram(code))) => {
                    let name = self.device.create_program(code.as_slice());
                    self.stream.send(ReplyNewProgram(name));
                },
                Err(comm::Empty) => break 'recv,
                Err(comm::Disconnected) => fail!("Render task has closed."),
            }
        }

        self.platform.swap_buffers();
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
