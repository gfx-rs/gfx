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

use device::shade::{ProgramMeta, Vertex, Fragment, UniformValue};
use self::envir::BindableStorage;
pub use BufferHandle = device::dev::Buffer;
pub type MeshHandle = uint;
pub type SurfaceHandle = device::dev::Surface;
pub type TextureHandle = device::dev::Texture;
pub type SamplerHandle = uint;
pub type ProgramHandle = uint;
pub type EnvirHandle = uint;

pub mod envir;
pub mod mesh;
pub mod target;


enum EnvirChangeRequest {
    EnvirBlock(envir::BlockVar, BufferHandle),
    EnvirUniform(envir::UniformVar, UniformValue),
    EnvirTexture(envir::TextureVar, TextureHandle, SamplerHandle),
}

enum Request {
    // Requests that require a reply:
    CallNewProgram(Vec<u8>, Vec<u8>),
    CallNewMesh(mesh::VertexCount, Vec<f32>, u8, u8),
    CallNewIndexBuffer(Vec<u16>),
    CallNewRawBuffer,
    CallNewEnvironment(envir::Storage),
    // Requests that don't expect a reply:
    CastClear(target::ClearData, Option<target::Frame>),
    CastDraw(MeshHandle, mesh::Slice, Option<target::Frame>, ProgramHandle, EnvirHandle),
    CastSetEnvironment(EnvirHandle, EnvirChangeRequest),
    CastUpdateBuffer(BufferHandle, Vec<f32>),
    CastEndFrame,
    CastFinish,
}

enum Reply {
    ReplyProgram(ProgramHandle),
    ReplyMesh(MeshHandle),
    ReplyBuffer(BufferHandle),
    ReplyEnvironment(EnvirHandle),
}

pub struct Client {
    stream: DuplexStream<Request, Reply>,
}

impl Client {
    pub fn clear(&self, data: target::ClearData, frame: Option<target::Frame>) {
        self.stream.send(CastClear(data, frame));
    }

    pub fn draw(&self, mesh: MeshHandle, slice: mesh::Slice, frame: Option<target::Frame>, program: ProgramHandle, env: EnvirHandle) {
        self.stream.send(CastDraw(mesh, slice, frame, program, env))
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

    pub fn create_mesh(&self, num_vert: mesh::VertexCount, data: Vec<f32>, count0: u8, stride0: u8) -> MeshHandle {
        self.stream.send(CallNewMesh(num_vert, data, count0, stride0));
        // TODO: delay recv()
        match self.stream.recv() {
            ReplyMesh(mesh) => mesh,
            _ => fail!("unknown reply")
        }
    }

    pub fn create_index_buffer(&self, data: Vec<u16>) -> BufferHandle {
        self.stream.send(CallNewIndexBuffer(data));
        // TODO: delay recv()
        match self.stream.recv() {
            ReplyBuffer(buffer) => buffer,
            _ => fail!("unknown reply")
        }
    }

    pub fn create_raw_buffer(&self) -> BufferHandle {
        self.stream.send(CallNewRawBuffer);
        match self.stream.recv() {
            ReplyBuffer(buffer) => buffer,
            _ => fail!("unknown reply")
        }
    }

    pub fn create_environment(&self, storage: envir::Storage) -> EnvirHandle {
        self.stream.send(CallNewEnvironment(storage));
        match self.stream.recv() {
            ReplyEnvironment(handle) => handle,
            _ => fail!("unknown reply")
        }
    }

    pub fn set_env_block(&self, env: EnvirHandle, var: envir::BlockVar, buf: BufferHandle) {
        self.stream.send(CastSetEnvironment(env, EnvirBlock(var, buf)));
    }

    pub fn set_env_uniform(&self, env: EnvirHandle, var: envir::UniformVar, value: UniformValue) {
        self.stream.send(CastSetEnvironment(env, EnvirUniform(var, value)));
    }

    pub fn set_env_texture(&self, env: EnvirHandle, var: envir::TextureVar, texture: TextureHandle, sampler: SamplerHandle) {
        self.stream.send(CastSetEnvironment(env, EnvirTexture(var, texture, sampler)));
    }

    pub fn update_buffer(&self, buf: BufferHandle, data: Vec<f32>) {
        self.stream.send(CastUpdateBuffer(buf, data));
    }
}


/// Temporary cache system before we get the handle manager
struct Cache {
    pub meshes: Vec<mesh::Mesh>,
    pub programs: Vec<ProgramMeta>,
    pub environments: Vec<envir::Storage>,
}

struct Server {
    no_send: marker::NoSend,
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
    device: device::Client2,
    /// a common VAO for mesh rendering
    common_array_buffer: device::dev::ArrayBuffer,
    /// the default FBO for drawing
    default_frame_buffer: device::dev::FrameBuffer,
    /// cached meta-data for meshes and programs
    cache: Cache,
}

impl Server {
    fn new(stream: DuplexStream<Reply, Request>, device: device::Client2) -> Server {
        device.send(device::CallNewArrayBuffer);
        let abuf = match device.recv() {
            device::ReplyNewArrayBuffer(name) => name,
            _ => fail!("invalid device reply for CallNewArrayBuffer")
        };
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
                environments: Vec::new(),
            },
        }
    }

    fn bind_frame(&mut self, frame_opt: &Option<target::Frame>) {
        match frame_opt {
            &Some(ref _frame) => {
                //TODO: find an existing FBO that matches the plane set
                // or create a new one and bind it
                unimplemented!()
            },
            &None => {
                self.device.send(device::CastBindFrameBuffer(self.default_frame_buffer));
            }
        }
    }

    fn bind_mesh(device: &mut device::Client2, mesh: &mesh::Mesh, prog: &ProgramMeta) -> Result<(),()> {
        for sat in prog.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => device.send(device::CastBindAttribute(sat.location as u8,
                    vat.buffer, vat.size as u32, vat.offset as u32, vat.stride as u32)),
                None => return Err(())
            }
        }
        Ok(())
    }

    fn bind_environment(device: &mut device::Client2, storage: &envir::Storage, shortcut: &envir::Shortcut, program: &ProgramMeta) {
        debug_assert!(storage.is_fit(shortcut, program));
        device.send(device::CastBindProgram(program.name));

        for (i, (&k, block_var)) in shortcut.blocks.iter().zip(program.blocks.iter()).enumerate() {
            let block = storage.get_block(k);
            block_var.active_slot.set(i as u8);
            device.send(device::CastBindUniformBlock(program.name, i as u8, i as device::UniformBufferSlot, block));
        }

        for (&k, uniform_var) in shortcut.uniforms.iter().zip(program.uniforms.iter()) {
            let value = storage.get_uniform(k);
            uniform_var.active_value.set(value);
            device.send(device::CastBindUniform(uniform_var.location, value));
        }

        for (_i, (&_k, _texture)) in shortcut.textures.iter().zip(program.textures.iter()).enumerate() {
            unimplemented!()
        }
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
                            self.device.send(device::CastClear(col));
                        },
                        None => unimplemented!()
                    }
                },
                Ok(CastDraw(mesh_handle, slice, frame, program_handle, env_handle)) => {
                    // bind output frame
                    self.bind_frame(&frame);
                    // bind shaders
                    let program = self.cache.programs.get(program_handle);
                    let env = self.cache.environments.get(env_handle);
                    match env.optimize(program) {
                        Ok(ref cut) => Server::bind_environment(&mut self.device, env, cut, program),
                        Err(err) => {
                            error!("Failed to build environment shortcut {}", err);
                            continue
                        },
                    }
                    // bind vertex attributes
                    self.device.send(device::CastBindArrayBuffer(self.common_array_buffer));
                    let mesh = self.cache.meshes.get(mesh_handle);
                    Server::bind_mesh(&mut self.device, mesh, program).unwrap();
                    // draw
                    match slice {
                        mesh::VertexSlice(start, end) => {
                            self.device.send(device::CastDraw(start, end));
                        },
                        mesh::IndexSlice(buf, start, end) => {
                            self.device.send(device::CastBindIndex(buf));
                            self.device.send(device::CastDrawIndexed(start, end));
                        },
                    }
                },
                Ok(CastSetEnvironment(handle, change)) => {
                    let env = self.cache.environments.get_mut(handle);
                    match change {
                        EnvirBlock(var, buf)                => env.set_block(var, buf),
                        EnvirUniform(var, value)            => env.set_uniform(var, value),
                        EnvirTexture(var, texture, sampler) => env.set_texture(var, texture, sampler),
                    }
                },
                Ok(CastUpdateBuffer(handle, data)) => {
                    self.device.send(device::CastUpdateBuffer(handle, data));
                },
                Ok(CastEndFrame) => {
                    self.device.send(device::CastSwapBuffers);
                },
                Ok(CallNewProgram(vs, fs)) => {
                    self.device.send(device::CallNewShader(Vertex, vs));
                    self.device.send(device::CallNewShader(Fragment, fs));
                    let h_vs = match self.device.recv() {
                        device::ReplyNewShader(name) => name.unwrap_or(0),
                        _ => fail!("invalid device reply for CallNewShader")
                    };
                    let h_fs = match self.device.recv() {
                        device::ReplyNewShader(name) => name.unwrap_or(0),
                        _ => fail!("invalid device reply for CallNewShader")
                    };
                    self.device.send(device::CallNewProgram(vec![h_vs, h_fs]));
                    let prog = match self.device.recv() {
                        device::ReplyNewProgram(Ok(prog)) => {
                            self.cache.programs.push(prog);
                            self.cache.programs.len() - 1
                        },
                        device::ReplyNewProgram(Err(_)) => 0,
                        _ => fail!("invalid device reply for CallNewProgram")
                    };
                    self.stream.send(ReplyProgram(prog));
                },
                Ok(CallNewMesh(num_vert, data, count, stride)) => {
                    self.device.send(device::CallNewVertexBuffer(data));
                    let buffer = match self.device.recv() {
                        device::ReplyNewBuffer(name) => name,
                        _ => fail!("invalid device reply for CallNewVertexBuffer")
                    };
                    let mut mesh = mesh::Mesh::new(num_vert);
                    mesh.attributes.push(mesh::Attribute {
                        buffer: buffer,
                        size: count,
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
                Ok(CallNewIndexBuffer(data)) => {
                    self.device.send(device::CallNewIndexBuffer(data));
                    let buffer = match self.device.recv() {
                        device::ReplyNewBuffer(name) => name,
                        _ => fail!("invalid device reply for CallNewIndexBuffer")
                    };
                    self.stream.send(ReplyBuffer(buffer));
                },
                Ok(CallNewRawBuffer) => {
                    self.device.send(device::CallNewRawBuffer);
                    let buffer = match self.device.recv() {
                        device::ReplyNewBuffer(name) => name,
                        _ => fail!("invalid device reply for CallNewRawBuffer")
                    };
                    self.stream.send(ReplyBuffer(buffer));
                },
                Ok(CallNewEnvironment(storage)) => {
                    let handle = self.cache.environments.len();
                    self.cache.environments.push(storage);
                    self.stream.send(ReplyEnvironment(handle));
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
pub fn start(_options: super::Options, device: device::Client2) -> Client {
    let (render_stream, task_stream) = comm::duplex::<Request, Reply>();
    spawn(proc() {
        let mut srv = Server::new(task_stream, device);
        while srv.update() {}
    });
    Client {
        stream: render_stream,
    }
}
