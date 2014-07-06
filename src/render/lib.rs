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

#![crate_id = "github.com/bjz/gfx-rs#render:0.1"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate comm;
extern crate device;

use std::sync::Future;

use device::shade::{ProgramMeta, Vertex, Fragment, UniformValue, ShaderSource};
use device::target::{ClearData, TargetColor, TargetDepth, TargetStencil};
use envir::BindableStorage;
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

/// Temporary cache system before we get the handle manager
struct Cache {
    pub meshes: Vec<mesh::Mesh>,
    pub programs: Vec<ProgramMeta>,
    pub environments: Vec<envir::Storage>,
}

/// Graphics state
struct State {
    frame: target::Frame,
}

#[deriving(Show)]
enum MeshError {
    ErrorMissingAttribute,
    ErrorAttributeType,
}


pub struct Renderer {
    device_tx: Sender<device::Request>,
    device_rx: Receiver<device::Reply>,
    swap_ack: Receiver<device::Ack>,
    should_finish: comm::ShouldClose,
    /// a common VAO for mesh rendering
    common_array_buffer: device::dev::ArrayBuffer,
    /// a common FBO for drawing
    common_frame_buffer: device::dev::FrameBuffer,
    /// the default FBO for drawing
    default_frame_buffer: device::dev::FrameBuffer,
    /// cached meta-data for meshes and programs
    cache: Cache,
    /// current state
    state: State,
}

impl Renderer {
    pub fn new(device_tx: Sender<device::Request>, device_rx: Receiver<device::Reply>,
            swap_rx: Receiver<device::Ack>, should_finish: comm::ShouldClose) -> Future<Renderer> {
        device_tx.send(device::CallNewArrayBuffer);
        device_tx.send(device::CallNewFrameBuffer);
        Future::from_fn(proc() {
            let array_buffer = match device_rx.recv() {
                device::ReplyNewArrayBuffer(array_buffer) => array_buffer,
                _ => fail!("invalid device reply for CallNewArrayBuffer"),
            };
            let frame_buffer = match device_rx.recv() {
                device::ReplyNewFrameBuffer(frame_buffer) => frame_buffer,
                _ => fail!("invalid device reply for CallNewFrameBuffer"),
            };
            Renderer {
                device_tx: device_tx,
                device_rx: device_rx,
                swap_ack: swap_rx,
                should_finish: should_finish,
                common_array_buffer: array_buffer,
                common_frame_buffer: frame_buffer,
                default_frame_buffer: 0,
                cache: Cache {
                    meshes: Vec::new(),
                    programs: Vec::new(),
                    environments: Vec::new(),
                },
                state: State {
                    frame: target::Frame::new(),
                },
            }
        })
    }

    pub fn should_finish(&self) -> bool {
        self.should_finish.check()
    }

    pub fn clear(&mut self, data: ClearData, frame: target::Frame) {
        self.bind_frame(&frame);
        self.device_tx.send(device::CastClear(data));
    }

    pub fn draw(&mut self, mesh_handle: MeshHandle, slice: mesh::Slice, frame: target::Frame, program_handle: ProgramHandle, env_handle: EnvirHandle) {
        // bind output frame
        self.bind_frame(&frame);
        // bind shaders
        let program = self.cache.programs.get(program_handle);
        let env = self.cache.environments.get(env_handle);
        match env.optimize(program) {
            Ok(ref cut) => self.bind_environment(env, cut, program),
            Err(err) => {
                error!("Failed to build environment shortcut {}", err);
                return;
            },
        }
        // bind vertex attributes
        self.device_tx.send(device::CastBindArrayBuffer(self.common_array_buffer));
        let mesh = self.cache.meshes.get(mesh_handle);
        self.bind_mesh(mesh, program).unwrap();
        // draw
        match slice {
            mesh::VertexSlice(start, end) => {
                self.device_tx.send(device::CastDraw(start, end));
            },
            mesh::IndexSlice(buf, start, end) => {
                self.device_tx.send(device::CastBindIndex(buf));
                self.device_tx.send(device::CastDrawIndexed(start, end));
            },
        }
    }

    pub fn end_frame(&self) {
        self.device_tx.send(device::CastSwapBuffers);
        self.swap_ack.recv();  //wait for acknowlegement
    }

    pub fn create_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource) -> ProgramHandle {
        self.device_tx.send(device::CallNewShader(Vertex, vs_src));
        self.device_tx.send(device::CallNewShader(Fragment, fs_src));
        let h_vs = match self.device_rx.recv() {
            device::ReplyNewShader(name) => name.unwrap_or(0),
            msg => fail!("invalid device reply for CallNewShader: {}", msg)
        };
        let h_fs = match self.device_rx.recv() {
            device::ReplyNewShader(name) => name.unwrap_or(0),
            msg => fail!("invalid device reply for CallNewShader: {}", msg)
        };
        self.device_tx.send(device::CallNewProgram(vec![h_vs, h_fs]));
        match self.device_rx.recv() {
            device::ReplyNewProgram(Ok(prog)) => {
                self.cache.programs.push(prog);
                self.cache.programs.len() - 1
            },
            device::ReplyNewProgram(Err(_)) => 0,
            _ => fail!("invalid device reply for CallNewProgram"),
        }
    }

    pub fn register_mesh(&mut self, mesh: mesh::Mesh) -> MeshHandle {
        let handle = self.cache.meshes.len();
        self.cache.meshes.push(mesh);
        handle
    }

    pub fn create_vertex_buffer(&self, data: Vec<f32>) -> BufferHandle {
        self.device_tx.send(device::CallNewVertexBuffer(data));
        match self.device_rx.recv() {
            device::ReplyNewBuffer(name) => name,
            _ => fail!("invalid device reply for CallNewVertexBuffer"),
        }
    }

    pub fn create_index_buffer(&self, data: Vec<u16>) -> BufferHandle {
        self.device_tx.send(device::CallNewIndexBuffer(data));
        match self.device_rx.recv() {
            device::ReplyNewBuffer(name) => name,
            _ => fail!("invalid device reply for CallNewIndexBuffer"),
        }
    }

    pub fn create_raw_buffer(&self) -> BufferHandle {
        self.device_tx.send(device::CallNewRawBuffer);
        match self.device_rx.recv() {
            device::ReplyNewBuffer(name) => name,
            _ => fail!("invalid device reply for CallNewRawBuffer"),
        }
    }

    pub fn create_environment(&mut self, storage: envir::Storage) -> EnvirHandle {
        let handle = self.cache.environments.len();
        self.cache.environments.push(storage);
        handle
    }

    pub fn set_env_block(&mut self, handle: EnvirHandle, var: envir::BlockVar, buf: BufferHandle) {
        self.cache.environments.get_mut(handle).set_block(var, buf);
    }

    pub fn set_env_uniform(&mut self, handle: EnvirHandle, var: envir::UniformVar, value: UniformValue) {
        self.cache.environments.get_mut(handle).set_uniform(var, value);
    }

    pub fn set_env_texture(&mut self, handle: EnvirHandle, var: envir::TextureVar, texture: TextureHandle, sampler: SamplerHandle) {
        self.cache.environments.get_mut(handle).set_texture(var, texture, sampler);
    }

    pub fn update_buffer(&self, buf: BufferHandle, data: Vec<f32>) {
        self.device_tx.send(device::CastUpdateBuffer(buf, data));
    }

    fn bind_frame(&mut self, frame: &target::Frame) {
        if frame.is_default() {
            // binding the default FBO, not touching our common one
            self.device_tx.send(device::CastBindFrameBuffer(self.default_frame_buffer));
        } else {
            self.device_tx.send(device::CastBindFrameBuffer(self.common_frame_buffer));
            for (i, (cur, new)) in self.state.frame.colors.mut_iter().zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    self.device_tx.send(device::CastBindTarget(TargetColor(i as u8), *new));
                }
            }
            if self.state.frame.depth != frame.depth {
                self.device_tx.send(device::CastBindTarget(TargetDepth, frame.depth));
            }
            if self.state.frame.stencil != frame.stencil {
                self.device_tx.send(device::CastBindTarget(TargetStencil, frame.stencil));
            }
            self.state.frame = *frame;
        }
    }

    fn bind_mesh(&self, mesh: &mesh::Mesh, prog: &ProgramMeta) -> Result<(),MeshError> {
        for sat in prog.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => match vat.elem_type.is_compatible(sat.base_type) {
                    Ok(_) => self.device_tx.send(device::CastBindAttribute(
                        sat.location as device::AttributeSlot, vat.buffer,
                        vat.elem_count, vat.elem_type, vat.stride, vat.offset)),
                    Err(_) => return Err(ErrorAttributeType)
                },
                None => return Err(ErrorMissingAttribute)
            }
        }
        Ok(())
    }

    fn bind_environment(&self, storage: &envir::Storage, shortcut: &envir::Shortcut, program: &ProgramMeta) {
        debug_assert!(storage.is_fit(shortcut, program));
        self.device_tx.send(device::CastBindProgram(program.name));

        for (i, (&k, block_var)) in shortcut.blocks.iter().zip(program.blocks.iter()).enumerate() {
            let block = storage.get_block(k);
            block_var.active_slot.set(i as u8);
            self.device_tx.send(device::CastBindUniformBlock(program.name, i as u8, i as device::UniformBufferSlot, block));
        }

        for (&k, uniform_var) in shortcut.uniforms.iter().zip(program.uniforms.iter()) {
            let value = storage.get_uniform(k);
            uniform_var.active_value.set(value);
            self.device_tx.send(device::CastBindUniform(uniform_var.location, value));
        }

        for (_i, (&_k, _texture)) in shortcut.textures.iter().zip(program.textures.iter()).enumerate() {
            unimplemented!()
        }
    }
}
