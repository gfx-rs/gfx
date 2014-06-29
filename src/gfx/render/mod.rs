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

use device::shade::{ProgramMeta, Vertex, Fragment};
pub type ProgramHandle = uint;
pub type MeshHandle = uint;
pub type Environment = ();  // placeholder

pub mod mesh;
pub mod target;


pub enum Request {
    // Requests that require a reply:
    CallNewProgram(Vec<u8>, Vec<u8>),
    CallNewMesh(mesh::VertexCount, Vec<f32>, u8),
    // Requests that don't expect a reply:
    CastClear(target::ClearData, Option<target::Frame>),
    CastDraw(MeshHandle, Option<target::Frame>, ProgramHandle),
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
    pub fn clear(&self, data: target::ClearData, frame: Option<target::Frame>) {
        self.stream.send(CastClear(data, frame));
    }

    pub fn draw(&self, mesh: MeshHandle, frame: Option<target::Frame>, program: ProgramHandle) {
        self.stream.send(CastDraw(mesh, frame, program))
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


/// Temporary cache system before we get the handle manager
struct Cache {
    pub meshes: Vec<mesh::Mesh>,
    pub programs: Vec<ProgramMeta>,
}

struct Server {
    no_send: marker::NoSend,
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
    device: device::Client,
    /// a common VAO for mesh rendering
    common_array_buffer: device::dev::ArrayBuffer,
    /// the default FBO for drawing
    default_frame_buffer: device::dev::FrameBuffer,
    /// cached meta-data for meshes and programs
    cache: Cache,
}

impl Server {
    fn new(stream: DuplexStream<Reply, Request>, device: device::Client) -> Server {
        let abuf = device.new_array_buffer();
        Server {
            no_send: marker::NoSend,
            no_share: marker::NoShare,
            stream: stream,
            device: device,
            common_array_buffer: abuf,
            default_frame_buffer: 0,
            cache: Cache {
                meshes: Vec::new(),
                programs: Vec::new(),
            },
        }
    }

    fn bind_frame(&mut self, frame_opt: &Option<target::Frame>) {
        match frame_opt {
            &Some(ref frame) => {
                //TODO: find an existing FBO that matches the plane set
                // or create a new one and bind it
                unimplemented!()
            },
            &None => {
                self.device.bind_frame_buffer(self.default_frame_buffer);
            }
        }
    }

    fn bind_mesh(device: &mut device::Client, mesh: &mesh::Mesh, prog: &ProgramMeta) -> Result<(),()> {
        for sat in prog.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => device.bind_attribute(sat.location as u8,
                    vat.buffer, mesh.num_vertices, vat.offset as u32, vat.stride as u32),
                None => return Err(())
            }
        }
        Ok(())
    }

    pub fn update(&mut self) -> bool {
        loop {
            match self.stream.try_recv() {
                Err(comm::Disconnected) | Ok(CastFinish) => {
                    return false; // terminate the rendering task
                                  // TODO: device.finish()?
                },
                Ok(CastClear(data, frame)) => {
                    self.bind_frame(&frame);
                    match data.color {
                        Some(col) => {
                            self.device.clear(col);
                        },
                        None => unimplemented!()
                    }
                },
                Ok(CastDraw(mesh_handle, frame, prog_handle)) => {
                    self.bind_frame(&frame);
                    self.device.bind_array_buffer(self.common_array_buffer);
                    let mesh = self.cache.meshes.get(mesh_handle);
                    let program = self.cache.programs.get(prog_handle);
                    Server::bind_mesh(&mut self.device, mesh, program).unwrap();
                    self.device.bind_program(program.name);
                    self.device.draw(0, mesh.num_vertices);
                },
                Ok(CastEndFrame) => {
                    self.device.end_frame();
                },
                Ok(CallNewProgram(vs, fs)) => {
                    let h_vs = self.device.new_shader(Vertex, vs);
                    let h_fs = self.device.new_shader(Fragment, fs);
                    let prog_opt = self.device.new_program(vec!(h_vs, h_fs));
                    let handle = match prog_opt {
                        Some(prog) => {
                            self.cache.programs.push(prog);
                            self.cache.programs.len()-1
                        },
                        None => 0,
                    };
                    self.stream.send(ReplyProgram(handle));
                },
                Ok(CallNewMesh(num_vert, data, stride)) => {
                    let buffer = self.device.new_buffer(data);
                    let mut mesh = mesh::Mesh::new(num_vert);
                    mesh.attributes.push(mesh::Attribute {
                        buffer: buffer,
                        offset: 0,
                        stride: stride,
                        is_normalized: false,
                        is_interpolated: false,
                        name: "a_Pos".to_string(),
                    });
                    let handle = self.cache.meshes.len();
                    self.cache.meshes.push(mesh);
                    self.stream.send(ReplyMesh(handle));
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
