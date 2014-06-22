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

use std::comm;
use std::comm::DuplexStream;
use std::kinds::marker;

use device;

pub use ProgramHandle = device::dev::Program;
pub use MeshHandle = self::mesh::Mesh;

pub mod mesh;


pub enum Request {
    // Requests that require a reply:
    CallNewProgram(Vec<u8>, Vec<u8>),
    CallNewMesh(mesh::VertexCount, Vec<f32>, u8),
    // Requests that don't expect a reply:
    CastClear(f32, f32, f32), // TODO: use color-rs?
    CastDraw(MeshHandle, ProgramHandle),
    CastEndFrame,
    CastFinish,
}

pub enum Reply {
    ReplyProgram(ProgramHandle),
    ReplyMesh(MeshHandle),
}

pub struct Client {
    stream: DuplexStream<Request, Reply>,
}

impl Client {
    pub fn clear(&self, r: f32, g: f32, b: f32) {
        self.stream.send(CastClear(r, g, b));
    }

    pub fn draw(&self, mesh: MeshHandle, program: ProgramHandle) {
        self.stream.send(CastDraw(mesh, program))
    }

    pub fn end_frame(&self) {
        self.stream.send(CastEndFrame)
    }

    pub fn finish(&self) {
        self.stream.send(CastFinish);
    }

    pub fn create_program(&self, vs_code: Vec<u8>, fs_code: Vec<u8>) -> ProgramHandle {
        self.stream.send(CallNewProgram(vs_code, fs_code));
        // TODO: delay recv()
        match self.stream.recv() {
            ReplyProgram(handle) => handle,
            _ => fail!("unknown reply")
        }
    }

    pub fn create_mesh(&self, num_vert: mesh::VertexCount, data: Vec<f32>, stride0: u8) -> MeshHandle {
        self.stream.send(CallNewMesh(num_vert, data, stride0));
        // TODO: delay recv()
        match self.stream.recv() {
            ReplyMesh(mesh) => mesh,
            _ => fail!("unknown reply")
        }
    }
}

struct Server {
    no_send: marker::NoSend,
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
    device: device::Client,
    /// a common VAO for mesh rendering
    array_buffer: device::dev::ArrayBuffer,

}

impl Server {
    fn new(stream: DuplexStream<Reply, Request>, device: device::Client) -> Server {
        let abuf = device.new_array_buffer();
        Server {
            no_send: marker::NoSend,
            no_share: marker::NoShare,
            stream: stream,
            device: device,
            array_buffer: abuf,
        }
    }

    pub fn update(&mut self) -> bool {
        loop {
            match self.stream.try_recv() {
                Err(comm::Disconnected) | Ok(CastFinish) => {
                    return false; // terminate the rendering task
                                  // TODO: device.finish()?
                },
                Ok(CastClear(r, g, b)) => {
                    self.device.clear(r, g, b);
                },
                Ok(CastDraw(mesh, program)) => {
                    self.device.bind_program(program);
                    self.device.bind_array_buffer(self.array_buffer);
                    for (i, at) in mesh.attributes.iter().enumerate().filter(|&(_,at)| at.buffer!=0) {
                        self.device.bind_attribute(i as u8, at.buffer, mesh.num_vertices,
                            at.offset as u32, at.stride as u32);
                    }
                    self.device.draw(0, mesh.num_vertices);
                },
                Ok(CastEndFrame) => {
                    self.device.end_frame();
                },
                Ok(CallNewProgram(vs, fs)) => {
                    let h_vs = self.device.new_shader('v', vs);
                    let h_fs = self.device.new_shader('f', fs);
                    let prog = self.device.new_program(vec!(h_vs, h_fs));
                    self.stream.send(ReplyProgram(prog));
                },
                Ok(CallNewMesh(num_vert, data, stride)) => {
                    let buffer = self.device.new_buffer(data);
                    let mut mesh = MeshHandle::new(num_vert);
                    mesh.attributes[0] = mesh::Attribute {
                        buffer: buffer,
                        offset: 0,
                        stride: stride,
                        is_normalized: false,
                        is_interpolated: false,
                        name: (),
                    };
                    self.stream.send(ReplyMesh(mesh));
                },
                Err(comm::Empty)  => {
                    break; // finished all the pending rendering messages
                },
            }
        }
        true
    }
}

/// Start a render server using the provided device client
pub fn start(_options: super::Options, device: device::Client) -> Client {
    let (render_stream, task_stream) = comm::duplex::<Request, Reply>();
    spawn(proc() {
        let mut srv = Server::new(task_stream, device);
        while srv.update() {}
    });
    Client {
        stream: render_stream,
    }
}
