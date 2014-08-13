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

//! High-level, platform independent, bindless rendering API.

#![crate_name = "render"]
#![comment = "A platform independent renderer for gfx-rs."]
#![license = "ASL2"]
#![crate_type = "lib"]
#![deny(missing_doc)]
#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate device;

use std::cell::Cell;
use std::fmt::Show;
use std::mem;
use std::vec::MoveItems;

use backend = device::dev;
use device::shade::{CreateShaderError, ProgramMeta, Vertex, Fragment, ShaderSource,
    UniformValue};
use device::target::{ClearData, Target, TargetColor, TargetDepth, TargetStencil};
use shade::{ProgramShell, ShaderParam};
use resource::{Loaded, Pending};

/// Used for sending/receiving handles to/from the device. Not meant for users.
#[experimental]
pub type Token = resource::Handle;
/// Shader handle
#[deriving(Clone, PartialEq, Show)]
pub struct ShaderHandle(Token);
/// Program handle
#[deriving(Clone, PartialEq, Show)]
pub struct ProgramHandle(Token);
/// Sampler handle
#[deriving(Clone, PartialEq, Show)]
pub struct SamplerHandle(Token);

/// Frontend
mod front;
/// Meshes
pub mod mesh;
/// Resources
pub mod resource;
/// Shaders
pub mod shade;
/// Draw state
pub mod state;
/// Render targets
pub mod target;

/// Graphics state
struct State {
    frame: target::Frame,
}

/// An error that can happen when sending commands to the device. Any attempt to use the handles
/// returned here will fail.
#[deriving(Clone, Show)]
pub enum DeviceError {
    /// Error creating a new buffer.
    ErrorNewBuffer(backend::Buffer),
    /// Error creating a new array buffer.
    ErrorNewArrayBuffer,
    /// Error creating a new shader.
    ErrorNewShader(ShaderHandle, CreateShaderError),
    /// Error creating a new program.
    ErrorNewProgram(ProgramHandle),
    /// Error creating a new framebuffer.
    ErrorNewFrameBuffer,
}

struct DeviceSender {
    chan: Sender<device::Request<Token>>,
    alive: Cell<bool>,
}

impl DeviceSender {
    fn send(&self, r: device::Request<Token>) {
        self.chan.send_opt(r).map_err(|_| self.alive.set(false));
    }

    fn is_alive(&self) -> bool {
        self.alive.get()
    }
}

/// A renderer. Methods on this get translated into commands for the device.
pub struct Renderer {
    device_tx: DeviceSender,
    swap_ack: Receiver<device::Ack>,
    should_close: bool,
    /// the shared VAO and FBO
    common_array_buffer: Token,
    common_frame_buffer: Token,
    /// the default FBO for drawing
    default_frame_buffer: backend::FrameBuffer,
    /// current state
    state: State,
}

type TempProgramResources = (
    Vec<Option<UniformValue>>,
    Vec<Option<backend::Buffer>>,
    Vec<Option<shade::TextureParam>>
);

impl Renderer {
    /// Create a new `Renderer` using given channels for communicating with the device. Generally,
    /// you want to use `gfx::start` instead.
    pub fn new(device_tx: Sender<device::Request<Token>>, device_rx: Receiver<device::Reply<Token>>,
            swap_rx: Receiver<device::Ack>) -> Renderer {
        // Request the creation of the common array buffer and frame buffer
        let mut res = resource::Cache::new();
        let c_vao = res.array_buffers.add(Pending);
        let c_fbo = res.frame_buffers.add(Pending);
        device_tx.send(device::Call(c_vao, device::CreateArrayBuffer));
        device_tx.send(device::Call(c_fbo, device::CreateFrameBuffer));
        // Return
        Renderer {
            device_tx: DeviceSender {
                chan: device_tx,
                alive: Cell::new(true),
            },
            swap_ack: swap_rx,
            should_close: false,
            common_array_buffer: c_vao,
            common_frame_buffer: c_fbo,
            default_frame_buffer: 0,
            state: State {
                frame: target::Frame::new(0, 0),
            },
        }
    }

    /// Ask the device to do something for us
    fn cast(&self, msg: device::CastRequest) {
        self.device_tx.send(device::Cast(msg));
    }

    /// Whether rendering should stop completely.
    pub fn should_finish(&self) -> bool {
        self.should_close || !self.device_tx.is_alive()
    }

    /// Finish rendering a frame. Waits for a frame to be finished drawing, as specified by the
    /// queue size passed to `gfx::start`.
    pub fn end_frame(&mut self) {
        self.device_tx.send(device::SwapBuffers);
        //wait for acknowlegement
        self.swap_ack.recv_opt().map_err(|_| {
            self.should_close = true; // the channel has disconnected, so it is time to close
        });
    }

    // --- Resource creation --- //

    /*/// Create a new program from the given vertex and fragment shaders.
    pub fn create_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource) -> ProgramHandle {
        let ds = &mut self.dispatcher;
        let h_vs = ds.resource.shaders.add(Pending);
        let h_fs = ds.resource.shaders.add(Pending);
        self.device_tx.send(device::Call(h_vs, device::CreateShader(Vertex, vs_src)));
        self.device_tx.send(device::Call(h_fs, device::CreateShader(Fragment, fs_src)));
        let token = ds.resource.programs.add(Pending);
        let shaders = vec![
            ds.get_shader(ShaderHandle(h_vs)),
            ds.get_shader(ShaderHandle(h_fs))
        ];
        self.device_tx.send(device::Call(token, device::CreateProgram(shaders)));
        ProgramHandle(token)
    }*/

    /*/// Create a new buffer on the device, which can be used to store vertex or uniform data.
    pub fn create_buffer<T: Send>(&mut self, data: Option<Vec<T>>) -> BufferHandle {
        let blob = data.map(|v| (box v) as Box<device::Blob + Send>);
        let token = self.dispatcher.resource.buffers.add(Pending);
        self.device_tx.send(device::Call(token, device::CreateBuffer(blob)));
        BufferHandle(token)
    }*/

    /*/// A helper method that returns a buffer handle that can never be used.
    /// It is needed for gfx_macros_test, which never actually accesses resources.
    #[cfg(test)]
    pub fn create_fake_buffer() -> BufferHandle {
        BufferHandle(resource::Handle::new_fake())
    }*/

    /*/// Create a new mesh from the given vertex data.
    ///
    /// Convenience function around `crate_buffer` and `Mesh::from`.
    pub fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh {
        let nv = data.len();
        debug_assert!(nv < { use std::num::Bounded; let val: device::VertexCount = Bounded::max_value(); val as uint });
        let buf = self.create_buffer(Some(data));
        mesh::Mesh::from::<T>(buf, nv as device::VertexCount)
    }*/

    /*/// Create a new surface.
    pub fn create_surface(&mut self, info: device::tex::SurfaceInfo) -> SurfaceHandle {
        let token = self.dispatcher.resource.surfaces.add((Pending, info.clone()));
        self.device_tx.send(device::Call(token, device::CreateSurface(info)));
        SurfaceHandle(token)
    }*/

    /*/// Create a new texture.
    pub fn create_texture(&mut self, info: device::tex::TextureInfo) -> TextureHandle {
        let token = self.dispatcher.resource.textures.add((Pending, info.clone()));
        self.device_tx.send(device::Call(token, device::CreateTexture(info)));
        TextureHandle(token)
    }*/

    /*/// Create a new sampler.
    pub fn create_sampler(&mut self, info: device::tex::SamplerInfo) -> SamplerHandle {
        let token = self.dispatcher.resource.samplers.add((Pending, info.clone()));
        self.device_tx.send(device::Call(token, device::CreateSampler(info)));
        SamplerHandle(token)
    }*/

    // --- Resource deletion --- //

    /*/// Delete a program
    pub fn delete_program(&mut self, handle: ProgramHandle) {
        let ProgramHandle(h) = handle;
        let v = self.dispatcher.resource.programs.remove(h).unwrap().unwrap().name;
        self.cast(device::DeleteProgram(v));
    }*/

    /*/// Delete a buffer
    pub fn delete_buffer(&mut self, handle: BufferHandle) {
        let BufferHandle(h) = handle;
        let v = *self.dispatcher.resource.buffers.remove(h).unwrap().unwrap();
        self.cast(device::DeleteBuffer(v));
    }*/

    /*/// Delete a surface
    pub fn delete_surface(&mut self, handle: SurfaceHandle) {
        let SurfaceHandle(h) = handle;
        let v = *self.dispatcher.resource.surfaces.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteSurface(v));
    }*/

    /*/// Delete a texture
    pub fn delete_texture(&mut self, handle: TextureHandle) {
        let TextureHandle(h) = handle;
        let v = *self.dispatcher.resource.textures.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteTexture(v));
    }*/

    /*/// Delete a sampler
    pub fn delete_sampler(&mut self, handle: SamplerHandle) {
        let SamplerHandle(h) = handle;
        let v = *self.dispatcher.resource.samplers.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteSampler(v));
    }*/

    // --- Resource modification --- //

    /*/// Connect a program together with its parameters.
    pub fn connect_program<'a, L, T: ShaderParam<L>>(&'a mut self, program: ProgramHandle, data: T)
                                                     -> Result<shade::CustomShell<L, T>,
                                                     shade::ParameterLinkError<'a>> {
        let ProgramHandle(ph) = program;
        self.dispatcher.demand(|res| !res.programs[ph].is_pending());
        match self.dispatcher.resource.programs.get(ph) {
            Ok(&Loaded(ref m)) => {
                let input = (m.uniforms.as_slice(), m.blocks.as_slice(), m.textures.as_slice());
                match data.create_link(input) {
                    Ok(link) => Ok(shade::CustomShell::new(program, link, data)),
                    Err(e) => Err(shade::ErrorUnusedParameter(e)),
                }
            },
            _ => Err(shade::ErrorBadProgram),
        }
    }*/

    /*/// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Send>(&mut self, handle: BufferHandle, data: Vec<T>) {
        let buf = self.dispatcher.get_buffer(handle);
        self.cast(device::UpdateBuffer(buf, (box data) as Box<device::Blob + Send>));
    }*/

    /*/// Update a buffer with data from a single type.
    pub fn update_buffer_struct<T: device::Blob+Send>(&mut self, handle: BufferHandle, data: T) {
        let buf = self.dispatcher.get_buffer(handle);
        self.cast(device::UpdateBuffer(buf, (box data) as Box<device::Blob + Send>));
    }*/

    /*/// Update the contents of a texture.
    pub fn update_texture<T: Send>(&mut self, tex: backend::Texture,
                                   img: device::tex::ImageInfo, data: Vec<T>) {
        let TextureHandle(tex) = handle;
        let name = *self.dispatcher.get_any(|res| res.textures[tex].ref0());
        let info = self.dispatcher.resource.textures[tex].ref1();
        self.cast(device::UpdateTexture(info.kind, name, img, (box data) as Box<device::Blob + Send>));
    }*/
}
