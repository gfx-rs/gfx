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

#![crate_name = "render"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate comm;
extern crate device;

use std::fmt::Show;

use backend = device::dev;
use device::shade::{ProgramMeta, Vertex, Fragment, UniformValue, ShaderSource, CreateShaderError};
use device::target::{ClearData, TargetColor, TargetDepth, TargetStencil};
use envir::BindableStorage;

pub type BufferHandle = uint;
pub type SurfaceHandle = backend::Surface;
pub type TextureHandle = backend::Texture;
pub type SamplerHandle = uint;
pub type ShaderHandle = uint;
pub type ProgramHandle = uint;
pub type EnvirHandle = uint;

pub mod envir;
pub mod mesh;
pub mod rast;
pub mod target;


pub type Token = uint;
pub type RequestChannel = Sender<device::Request<Token>>;

#[deriving(PartialEq, Show)]
enum MaybeLoaded<R, E> {
    Pending,
    Loaded(R),
    Failed(E),
}

impl<R, E: Show> MaybeLoaded<R, E> {
    fn is_loaded(&self) -> bool {
        match *self {
            Pending => false,
            _ => true,
        }
    }

    fn unwrap<'a>(&'a self) -> &'a R {
        match *self {
            Pending => fail!("Resource not loaded yet"),
            Loaded(ref res) => res,
            Failed(ref e) => fail!("Resource load fail: {}", e),
        }
    }
}

type ResourceVec<R, E> = Vec<MaybeLoaded<R, E>>;

/// Storage for all loaded objects
struct ResourceCache {
    pub buffers: ResourceVec<backend::Buffer, ()>,
    pub array_buffers: ResourceVec<backend::ArrayBuffer, ()>,
    pub shaders: ResourceVec<backend::Shader, CreateShaderError>,
    pub programs: ResourceVec<ProgramMeta, ()>,
    pub frame_buffers: ResourceVec<backend::FrameBuffer, ()>,
}

impl ResourceCache {
    fn new() -> ResourceCache {
        ResourceCache {
            buffers: Vec::new(),
            array_buffers: Vec::new(),
            shaders: Vec::new(),
            programs: Vec::new(),
            frame_buffers: Vec::new(),
        }
    }

    fn process(&mut self, reply: device::Reply<Token>) {
        match reply {
            device::ReplyNewBuffer(token, buf) => {
                *self.buffers.get_mut(token) = Loaded(buf);
            },
            device::ReplyNewArrayBuffer(token, result) => {
                *self.array_buffers.get_mut(token) = match result {
                    Ok(vao) => Loaded(vao),
                    Err(e) => Failed(e),
                };
            },
            device::ReplyNewShader(token, result) => {
                *self.shaders.get_mut(token) = match result {
                    Ok(sh) => Loaded(sh),
                    Err(e) => Failed(e),
                };
            },
            device::ReplyNewProgram(token, result) => {
                *self.programs.get_mut(token) = match result {
                    Ok(prog) => Loaded(prog),
                    Err(e) => Failed(e),
                };
            },
            device::ReplyNewFrameBuffer(token, fbo) => {
                *self.frame_buffers.get_mut(token) = Loaded(fbo);
            },
        }        
    }
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
    device_tx: RequestChannel,
    device_rx: Receiver<device::Reply<Token>>,
    swap_ack: Receiver<device::Ack>,
    should_finish: comm::ShouldClose,
    /// the default FBO for drawing
    default_frame_buffer: backend::FrameBuffer,
    /// cached meta-data for meshes and programs
    resource: ResourceCache,
    environments: Vec<envir::Storage>,
    /// current state
    state: State,
}

/// Resource-oriented private methods
impl Renderer {
    /// Make sure the resource is loaded
    fn demand(&mut self, fn_ready: |&ResourceCache| -> bool) {
        while !fn_ready(&self.resource) {
            let reply = self.device_rx.recv();
            self.resource.process(reply);
        }
    }

    fn get_buffer(&mut self, handle: BufferHandle) -> backend::Buffer {
        self.demand(|res| res.buffers.get(handle).is_loaded());
        *self.resource.buffers.get(handle).unwrap()
    }

    fn get_common_array_buffer(&mut self) -> backend::ArrayBuffer {
        self.demand(|res| res.array_buffers.get(0).is_loaded());
        *self.resource.array_buffers.get(0).unwrap()
    }

    fn get_shader(&mut self, handle: ShaderHandle) -> backend::Shader {
        self.demand(|res| res.shaders.get(handle).is_loaded());
        *self.resource.shaders.get(handle).unwrap()
    }

    fn get_common_frame_buffer(&mut self) -> backend::FrameBuffer {
        self.demand(|res| res.frame_buffers.get(0).is_loaded());
        *self.resource.frame_buffers.get(0).unwrap()
    }
}

/// Graphics-oriented methods
impl Renderer {
    pub fn new(device_tx: RequestChannel, device_rx: Receiver<device::Reply<Token>>,
            swap_rx: Receiver<device::Ack>, should_finish: comm::ShouldClose) -> Renderer {
        // Request the creation of the common array buffer and frame buffer
        let mut res = ResourceCache::new();
        res.array_buffers.push(Pending);
        res.frame_buffers.push(Pending);
        device_tx.send(device::Call(0, device::CreateArrayBuffer));
        device_tx.send(device::Call(0, device::CreateFrameBuffer));
        // Return
        Renderer {
            device_tx: device_tx,
            device_rx: device_rx,
            swap_ack: swap_rx,
            should_finish: should_finish,
            default_frame_buffer: 0,
            resource: res,
            environments: Vec::new(),
            state: State {
                frame: target::Frame::new(),
            },
        }
    }

    fn call(&self, token: Token, msg: device::CallRequest) {
        self.device_tx.send(device::Call(token, msg));
    }

    fn cast(&self, msg: device::CastRequest) {
        self.device_tx.send(device::Cast(msg));
    }

    pub fn should_finish(&self) -> bool {
        self.should_finish.check()
    }

    pub fn clear(&mut self, data: ClearData, frame: target::Frame) {
        self.bind_frame(&frame);
        self.cast(device::Clear(data));
    }

    pub fn draw(&mut self, mesh: &mesh::Mesh, slice: mesh::Slice, frame: target::Frame,
            program_handle: ProgramHandle, env_handle: EnvirHandle, state: rast::DrawState) {
        // demand resources
        self.prebind_mesh(mesh);
        self.demand(|res| res.programs.get(program_handle).is_loaded());
        // bind state
        self.cast(device::SetPrimitiveState(state.primitive));
        self.cast(device::SetDepthStencilState(state.depth, state.stencil,
            state.primitive.get_cull_mode()));
        self.cast(device::SetBlendState(state.blend));
        // bind array buffer
        let vao = self.get_common_array_buffer();
        self.cast(device::BindArrayBuffer(vao));
        // bind output frame
        self.bind_frame(&frame);
        // bind shaders
        let env = self.environments.get(env_handle);
        // prebind the environment - unable to make it a method of self
        for handle in env.iter_buffers() {
            while !self.resource.buffers.get(handle).is_loaded() {
                let reply = self.device_rx.recv();
                self.resource.process(reply);
            }
        }
        let program = self.resource.programs.get(program_handle).unwrap();
        match env.optimize(program) {
            Ok(ref cut) => self.bind_environment(env, cut, program),
            Err(err) => {
                error!("Failed to build environment shortcut {}", err);
                return;
            },
        }
        // bind vertex attributes
        self.bind_mesh(mesh, program).unwrap();
        // draw
        match slice {
            mesh::VertexSlice(start, end) => {
                self.cast(device::Draw(start, end));
            },
            mesh::IndexSlice(buf, start, end) => {
                self.cast(device::BindIndex(buf));
                self.cast(device::DrawIndexed(start, end));
            },
        }
    }

    pub fn end_frame(&self) {
        self.device_tx.send(device::SwapBuffers);
        self.swap_ack.recv();  //wait for acknowlegement
    }

    pub fn create_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource) -> ProgramHandle {
        let id = self.resource.shaders.len();
        self.resource.shaders.push(Pending);
        self.resource.shaders.push(Pending);
        self.call(id+0, device::CreateShader(Vertex, vs_src));
        self.call(id+1, device::CreateShader(Fragment, fs_src));
        let h_vs = self.get_shader(id+0);
        let h_fs = self.get_shader(id+1);
        let token = self.resource.programs.len();
        self.call(token, device::CreateProgram(vec![h_vs, h_fs]));
        self.resource.programs.push(Pending);
        token
    }

    pub fn create_vertex_buffer(&mut self, data: Vec<f32>) -> BufferHandle {
        let token = self.resource.buffers.len();
        self.call(token, device::CreateVertexBuffer(data));
        self.resource.buffers.push(Pending);
        token
    }

    pub fn create_index_buffer(&mut self, data: Vec<u16>) -> BufferHandle {
        let token = self.resource.buffers.len();
        self.call(token, device::CreateIndexBuffer(data));
        self.resource.buffers.push(Pending);
        token
    }

    pub fn create_raw_buffer(&mut self) -> BufferHandle {
        let token = self.resource.buffers.len();
        self.call(token, device::CreateRawBuffer);
        self.resource.buffers.push(Pending);
        token
    }

    pub fn create_environment(&mut self, storage: envir::Storage) -> EnvirHandle {
        let handle = self.environments.len();
        self.environments.push(storage);
        handle
    }

    pub fn set_env_block(&mut self, handle: EnvirHandle, var: envir::BlockVar, buf: BufferHandle) {
        self.environments.get_mut(handle).set_block(var, buf);
    }

    pub fn set_env_uniform(&mut self, handle: EnvirHandle, var: envir::UniformVar, value: UniformValue) {
        self.environments.get_mut(handle).set_uniform(var, value);
    }

    pub fn set_env_texture(&mut self, handle: EnvirHandle, var: envir::TextureVar, texture: TextureHandle, sampler: SamplerHandle) {
        self.environments.get_mut(handle).set_texture(var, texture, sampler);
    }

    pub fn update_buffer(&mut self, handle: BufferHandle, data: Vec<f32>) {
        let buf = self.get_buffer(handle);
        self.cast(device::UpdateBuffer(buf, data));
    }

    fn bind_frame(&mut self, frame: &target::Frame) {
        if frame.is_default() {
            // binding the default FBO, not touching our common one
            self.cast(device::BindFrameBuffer(self.default_frame_buffer));
        } else {
            let fbo = self.get_common_frame_buffer();
            self.cast(device::BindFrameBuffer(fbo));
            for (i, (cur, new)) in self.state.frame.colors.iter().zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    self.cast(device::BindTarget(TargetColor(i as u8), *new));
                }
            }
            if self.state.frame.depth != frame.depth {
                self.cast(device::BindTarget(TargetDepth, frame.depth));
            }
            if self.state.frame.stencil != frame.stencil {
                self.cast(device::BindTarget(TargetStencil, frame.stencil));
            }
            self.state.frame = *frame;
        }
    }

    fn prebind_mesh(&mut self, mesh: &mesh::Mesh) {
        for at in mesh.attributes.iter() {
            self.get_buffer(at.buffer);
        }
    }

    fn bind_mesh(&self, mesh: &mesh::Mesh, prog: &ProgramMeta) -> Result<(),MeshError> {
        for sat in prog.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => match vat.elem_type.is_compatible(sat.base_type) {
                    Ok(_) => self.cast(device::BindAttribute(
                        sat.location as device::AttributeSlot,
                        *self.resource.buffers.get(vat.buffer).unwrap(),
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
        self.cast(device::BindProgram(program.name));

        for (i, (&k, block_var)) in shortcut.blocks.iter().zip(program.blocks.iter()).enumerate() {
            let handle = storage.get_block(k);
            let block = *self.resource.buffers.get(handle).unwrap();
            block_var.active_slot.set(i as u8);
            self.cast(device::BindUniformBlock(program.name, i as u8, i as device::UniformBufferSlot, block));
        }

        for (&k, uniform_var) in shortcut.uniforms.iter().zip(program.uniforms.iter()) {
            let value = storage.get_uniform(k);
            uniform_var.active_value.set(value);
            self.cast(device::BindUniform(uniform_var.location, value));
        }

        for (_i, (&_k, _texture)) in shortcut.textures.iter().zip(program.textures.iter()).enumerate() {
            unimplemented!()
        }
    }
}
