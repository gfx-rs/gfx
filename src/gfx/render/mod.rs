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

use server;
use device;

pub use ProgramHandle = device::dev::Program;
pub struct MeshHandle {
    vertex_buf: device::dev::Buffer,
    array_buffer: device::dev::ArrayBuffer,
}


pub enum Call {
    CallNewProgram(Vec<u8>, Vec<u8>),
    CallNewMesh(Vec<f32>),
}

pub enum Cast {
    CastClear(f32, f32, f32), // TODO: use color-rs?
    CastEndFrame,
    CastFinish,
}
pub type Request = server::Request<Call, Cast>;

pub enum Reply {
    ReplyProgram(ProgramHandle),
    ReplyMesh(MeshHandle),
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
        self.cast(CastClear(r, g, b));
    }

    pub fn end_frame(&self) {
        self.cast(CastEndFrame)
    }

    pub fn finish(&self) {
        self.cast(CastFinish);
    }

    pub fn create_program(&self, vs_code: Vec<u8>, fs_code: Vec<u8>) -> ProgramHandle {
        match self.call(CallNewProgram(vs_code, fs_code)) {
            ReplyProgram(handle) => handle,
            _ => fail!("unknown reply")
        }
    }

    pub fn create_mesh(&self, data: Vec<f32>) -> MeshHandle {
        match self.call(CallNewMesh(data)) {
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
        'recv: loop {
            match self.stream.try_recv() {
                Err(comm::Disconnected) | Ok(server::Cast(CastFinish)) => {
                    return false; // terminate the rendering task
                                  // TODO: device.finish()?
                },
                Ok(server::Cast(CastClear(r, g, b))) => {
                    self.device.clear(r, g, b);
                },
                Ok(server::Cast(CastEndFrame)) => {
                    self.device.end_frame();
                },
                Ok(server::Call(CallNewProgram(vs, fs))) => {
                    let h_vs = self.device.new_shader('v', vs);
                    let h_fs = self.device.new_shader('f', fs);
                    let prog = self.device.new_program(vec!(h_vs, h_fs));
                    self.stream.send(ReplyProgram(prog));
                },
                Ok(server::Call(CallNewMesh(data))) => {
                    let buffer = self.device.new_buffer(data);
                    let mesh = MeshHandle {
                        vertex_buf: buffer,
                        array_buffer: self.array_buffer,
                    };
                    self.stream.send(ReplyMesh(mesh));
                },
                Err(comm::Empty)  => {
                    break 'recv; // finished all the pending rendering messages
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
