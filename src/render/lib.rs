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
    UniformValue, ValueUninitialized};
use device::target::{ClearData, Target, TargetColor, TargetDepth, TargetStencil};
use shade::{ProgramShell, ShaderParam};
use resource::{Loaded, Pending};

/// Used for sending/receiving handles to/from the device. Not meant for users.
#[experimental]
pub type Token = resource::Handle;
/// Buffer handle
#[deriving(Clone, PartialEq, Show)]
pub struct BufferHandle(Token);
/// Shader handle
#[deriving(Clone, PartialEq, Show)]
pub struct ShaderHandle(Token);
/// Program handle
#[deriving(Clone, PartialEq, Show)]
pub struct ProgramHandle(Token);
/// Surface handle
#[deriving(Clone, PartialEq, Show)]
pub struct SurfaceHandle(Token);
/// Texture handle
#[deriving(Clone, PartialEq, Show)]
pub struct TextureHandle(Token);
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
    ErrorNewBuffer(BufferHandle),
    /// Error creating a new array buffer.
    ErrorNewArrayBuffer,
    /// Error creating a new shader.
    ErrorNewShader(ShaderHandle, CreateShaderError),
    /// Error creating a new program.
    ErrorNewProgram(ProgramHandle),
    /// Error creating a new framebuffer.
    ErrorNewFrameBuffer,
}

/// An error with an invalid texture or uniform block.
//TODO: use slices when Rust allows
#[deriving(Show)]
pub enum ShellError {
    /// Error from a uniform value
    ErrorShellUniform(String),
    /// Error from a uniform block.
    ErrorShellBlock(String),
    /// Error from a texture.
    ErrorShellTexture(String),
    /// Error from a sampler
    ErrorShellSampler(String),
}

/// An error with a defined Mesh.
#[deriving(Show)]
pub enum MeshError {
    /// A required attribute was missing.
    ErrorAttributeMissing,
    /// An attribute's type from the vertex format differed from the type used in the shader.
    ErrorAttributeType,
}

/// An error that can happen when trying to draw.
#[deriving(Show)]
pub enum DrawError {
    /// Error with a program.
    ErrorProgram,
    /// Error with the program shell.
    ErrorShell(ShellError),
    /// Error with the mesh.
    ErrorMesh(MeshError),
	/// Error with the mesh slice
    ErrorSlice,
}

struct Dispatcher {
    /// Channel to receive device messages
    channel: Receiver<device::Reply<Token>>,
    /// Alive status
    is_alive: bool,
    /// Asynchronous device error queue
    errors: Vec<DeviceError>,
    /// cached meta-data for meshes and programs
    resource: resource::Cache,
}

impl Dispatcher {
    /// Make sure the resource is loaded. Optimally, we'd like this method to return
    /// the resource reference, but the borrow checker doesn't like the match over `Future`
    /// inside the body.
    fn demand(&mut self, fn_ready: |&resource::Cache| -> bool) {
        while !fn_ready(&self.resource) {
            let reply = match self.channel.recv_opt() {
                Ok(r) => r,
                Err(_) => {
                    self.is_alive = false;
                    return;
                },
            };
            match self.resource.process(reply) {
                Ok(_) => (),
                Err(e) => self.errors.push(e),
            }
        }
    }

    /// Get a guaranteed copy of a specific resource accessed by the function.
    fn get_any<R, E: Show>(&mut self, fun: <'a>|&'a resource::Cache| -> &'a resource::Future<R, E>) -> &R {
        self.demand(|res| !fun(res).is_pending());
        fun(&self.resource).unwrap()
    }

    fn get_buffer(&mut self, handle: BufferHandle) -> backend::Buffer {
        let BufferHandle(h) = handle;
        *self.get_any(|res| &res.buffers[h])
    }

    fn get_shader(&mut self, handle: ShaderHandle) -> backend::Shader {
        let ShaderHandle(h) = handle;
        *self.get_any(|res| &res.shaders[h])
    }
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
    dispatcher: Dispatcher,
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
    Vec<Option<BufferHandle>>,
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
            dispatcher: Dispatcher {
                channel: device_rx,
                is_alive: true,
                errors: Vec::new(),
                resource: res,
            },
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
        self.should_close || !self.device_tx.is_alive() || !self.dispatcher.is_alive
    }

    /// Iterate over any errors that have been raised by the device when trying to issue commands
    /// since the last time this method was called.
    pub fn errors(&mut self) -> MoveItems<DeviceError> {
        mem::replace(&mut self.dispatcher.errors, Vec::new()).move_iter()
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ClearData, frame: target::Frame) {
        self.bind_frame(&frame);
        self.cast(device::Clear(data));
    }

    /// Draw `slice` of `mesh` into `frame`, using a `bundle` of shader program and parameters, and
    /// a given draw state.
    pub fn draw<P: ProgramShell>(&mut self, mesh: &mesh::Mesh, slice: mesh::Slice,
                                 frame: &target::Frame, prog_shell: &P, state: &state::DrawState)
                                 -> Result<(), DrawError> {
        // demand resources. This section needs the mutable self, so we are
        // unable to do this after we get a reference to any resource
        self.prebind_mesh(mesh, &slice);
        let ProgramHandle(ph) = prog_shell.get_program();
        self.dispatcher.demand(|res| !res.programs[ph].is_pending());
        // obtain program parameter values & resources
        let parameters = match self.prebind_shell(prog_shell) {
            Ok(params) => params,
            Err(e) => return Err(e),
        };
        // bind state
        self.cast(device::SetPrimitiveState(state.primitive));
        self.cast(device::SetScissor(state.scissor));
        self.cast(device::SetDepthStencilState(state.depth, state.stencil,
            state.primitive.get_cull_mode()));
        self.cast(device::SetBlendState(state.blend));
        self.cast(device::SetColorMask(state.color_mask));
        // bind array buffer
        let h_vao = self.common_array_buffer;
        let vao = *self.dispatcher.get_any(|res| &res.array_buffers[h_vao]);
        self.cast(device::BindArrayBuffer(vao));
        // bind output frame
        self.bind_frame(frame);
        // bind shaders
        let program = self.dispatcher.resource.programs[ph].unwrap();
        match self.bind_shell(program, parameters) {
            Ok(_) => (),
            Err(e) => return Err(ErrorShell(e)),
        }
        // bind vertex attributes
        match self.bind_mesh(mesh, program) {
            Ok(_) => (),
            Err(e) => return Err(ErrorMesh(e)),
        }
        // draw
        match slice {
            mesh::VertexSlice(start, end) => {
                self.cast(device::Draw(mesh.prim_type, start, end));
            },
            mesh::IndexSlice(handle, index, start, end) => {
                let BufferHandle(bh) = handle;
                let buf = match self.dispatcher.resource.buffers.get(bh) {
                    Ok(&Loaded(buf)) => buf,
                    _ => return Err(ErrorSlice),
                };
                self.cast(device::BindIndex(buf));
                self.cast(device::DrawIndexed(mesh.prim_type, index, start, end));
            },
        }
        Ok(())
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

    /// Create a new program from the given vertex and fragment shaders.
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
    }

    /// Create a new buffer on the device, which can be used to store vertex or uniform data.
    pub fn create_buffer<T: Send>(&mut self, data: Option<Vec<T>>) -> BufferHandle {
        let blob = data.map(|v| (box v) as Box<device::Blob + Send>);
        let token = self.dispatcher.resource.buffers.add(Pending);
        self.device_tx.send(device::Call(token, device::CreateBuffer(blob)));
        BufferHandle(token)
    }

    /// A helper method that returns a buffer handle that can never be used.
    /// It is needed for gfx_macros_test, which never actually accesses resources.
    #[cfg(test)]
    pub fn create_fake_buffer() -> BufferHandle {
        BufferHandle(resource::Handle::new_fake())
    }

    /// Create a new mesh from the given vertex data.
    ///
    /// Convenience function around `crate_buffer` and `Mesh::from`.
    pub fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh {
        let nv = data.len();
        debug_assert!(nv < { use std::num::Bounded; let val: device::VertexCount = Bounded::max_value(); val as uint });
        let buf = self.create_buffer(Some(data));
        mesh::Mesh::from::<T>(buf, nv as device::VertexCount)
    }

    /// Create a new surface.
    pub fn create_surface(&mut self, info: device::tex::SurfaceInfo) -> SurfaceHandle {
        let token = self.dispatcher.resource.surfaces.add((Pending, info.clone()));
        self.device_tx.send(device::Call(token, device::CreateSurface(info)));
        SurfaceHandle(token)
    }

    /// Create a new texture.
    pub fn create_texture(&mut self, info: device::tex::TextureInfo) -> TextureHandle {
        let token = self.dispatcher.resource.textures.add((Pending, info.clone()));
        self.device_tx.send(device::Call(token, device::CreateTexture(info)));
        TextureHandle(token)
    }

    /// Create a new sampler.
    pub fn create_sampler(&mut self, info: device::tex::SamplerInfo) -> SamplerHandle {
        let token = self.dispatcher.resource.samplers.add((Pending, info.clone()));
        self.device_tx.send(device::Call(token, device::CreateSampler(info)));
        SamplerHandle(token)
    }

    // --- Resource deletion --- //

    /// Delete a program
    pub fn delete_program(&mut self, handle: ProgramHandle) {
        let ProgramHandle(h) = handle;
        let v = self.dispatcher.resource.programs.remove(h).unwrap().unwrap().name;
        self.cast(device::DeleteProgram(v));
    }

    /// Delete a buffer
    pub fn delete_buffer(&mut self, handle: BufferHandle) {
        let BufferHandle(h) = handle;
        let v = *self.dispatcher.resource.buffers.remove(h).unwrap().unwrap();
        self.cast(device::DeleteBuffer(v));
    }

    /// Delete a surface
    pub fn delete_surface(&mut self, handle: SurfaceHandle) {
        let SurfaceHandle(h) = handle;
        let v = *self.dispatcher.resource.surfaces.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteSurface(v));
    }

    /// Delete a texture
    pub fn delete_texture(&mut self, handle: TextureHandle) {
        let TextureHandle(h) = handle;
        let v = *self.dispatcher.resource.textures.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteTexture(v));
    }

    /// Delete a sampler
    pub fn delete_sampler(&mut self, handle: SamplerHandle) {
        let SamplerHandle(h) = handle;
        let v = *self.dispatcher.resource.samplers.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteSampler(v));
    }

    // --- Resource modification --- //

    /// Connect a program together with its parameters.
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
    }

    /// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Send>(&mut self, handle: BufferHandle, data: Vec<T>) {
        let buf = self.dispatcher.get_buffer(handle);
        self.cast(device::UpdateBuffer(buf, (box data) as Box<device::Blob + Send>));
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<T: device::Blob+Send>(&mut self, handle: BufferHandle, data: T) {
        let buf = self.dispatcher.get_buffer(handle);
        self.cast(device::UpdateBuffer(buf, (box data) as Box<device::Blob + Send>));
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Send>(&mut self, handle: TextureHandle,
                                   img: device::tex::ImageInfo, data: Vec<T>) {
        let TextureHandle(tex) = handle;
        let name = *self.dispatcher.get_any(|res| res.textures[tex].ref0());
        let info = self.dispatcher.resource.textures[tex].ref1();
        self.cast(device::UpdateTexture(info.kind, name, img, (box data) as Box<device::Blob + Send>));
    }

    // --- Resource binding --- //

    /// Make sure all the mesh buffers are successfully created/loaded
    fn prebind_mesh(&mut self, mesh: &mesh::Mesh, slice: &mesh::Slice) {
        for at in mesh.attributes.iter() {
            self.dispatcher.get_buffer(at.buffer);
        }
        match *slice {
            mesh::IndexSlice(handle, _, _, _) =>
                self.dispatcher.get_buffer(handle),
            _ => 0,
        };
    }

    fn prebind_shell<P: ProgramShell>(&mut self, shell: &P)
            -> Result<TempProgramResources, DrawError> {
        let dp = &mut self.dispatcher;
        let parameters = {
            let ProgramHandle(ph) = shell.get_program();
            let meta = match dp.resource.programs.get(ph) {
                Ok(&resource::Loaded(ref m)) => m,
                _ => return Err(ErrorProgram),
            };
            //TODO: pre-allocate these vectors and re-use them
            let mut uniforms = Vec::from_elem(meta.uniforms.len(), None);
            let mut blocks   = Vec::from_elem(meta.blocks.len(), None);
            let mut textures = Vec::from_elem(meta.textures.len(), None);
            shell.fill_params(shade::ParamValues {
                uniforms: uniforms.as_mut_slice(),
                blocks: blocks.as_mut_slice(),
                textures: textures.as_mut_slice(),
            });
            // verify that all the parameters were written
            match uniforms.iter().zip(meta.uniforms.iter()).find(|&(u, _)| u.is_none()) {
                Some((_, var)) => return Err(ErrorShell(ErrorShellUniform(var.name.clone()))),
                None => (),
            }
            match blocks.iter().zip(meta.blocks.iter()).find(|&(b, _)| b.is_none()) {
                Some((_, var)) => return Err(ErrorShell(ErrorShellBlock(var.name.clone()))),
                None => (),
            }
            match textures.iter().zip(meta.textures.iter()).find(|&(t, _)| t.is_none()) {
                Some((_, var)) => return Err(ErrorShell(ErrorShellTexture(var.name.clone()))),
                None => (),
            }
            (uniforms, blocks, textures)
        };
        // buffers pass
        for option in parameters.ref1().iter() {
            let BufferHandle(buf) = option.unwrap();
            dp.demand(|res| !res.buffers[buf].is_pending());
        }
        // texture pass
        for option in parameters.ref2().iter() {
            let (TextureHandle(tex), sampler) = option.unwrap();
            dp.demand(|res| !res.textures[tex].ref0().is_pending());
            match sampler {
                Some(SamplerHandle(sam)) =>
                    dp.demand(|res| !res.samplers[sam].ref0().is_pending()),
                None => (),
            }
        }
        // done
        Ok(parameters)
    }

    fn make_target_cast(dp: &mut Dispatcher, to: Target, plane: target::Plane) -> device::CastRequest {
        match plane {
            target::PlaneEmpty => device::UnbindTarget(to),
            target::PlaneSurface(SurfaceHandle(suf)) => {
                let name = *dp.get_any(|res| res.surfaces[suf].ref0());
                device::BindTargetSurface(to, name)
            },
            target::PlaneTexture(TextureHandle(tex), level, layer) => {
                let name = *dp.get_any(|res| res.textures[tex].ref0());
                device::BindTargetTexture(to, name, level, layer)
            },
        }
    }

    fn bind_frame(&mut self, frame: &target::Frame) {
        self.cast(device::SetViewport(device::target::Rect {
            x: 0,
            y: 0,
            w: frame.width,
            h: frame.height,
        }));
        if frame.is_default() {
            // binding the default FBO, not touching our common one
            self.cast(device::BindFrameBuffer(self.default_frame_buffer));
        } else {
            let h_fbo = self.common_frame_buffer;
            let fbo = *self.dispatcher.get_any(|res| &res.frame_buffers[h_fbo]);
            self.cast(device::BindFrameBuffer(fbo));
            for (i, (cur, new)) in self.state.frame.colors.iter().zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    let msg = Renderer::make_target_cast(&mut self.dispatcher, TargetColor(i as u8), *new);
                    self.cast(msg);
                }
            }
            if self.state.frame.depth != frame.depth {
                let msg = Renderer::make_target_cast(&mut self.dispatcher, TargetDepth, frame.depth);
                self.cast(msg);
            }
            if self.state.frame.stencil != frame.stencil {
                let msg = Renderer::make_target_cast(&mut self.dispatcher, TargetStencil, frame.stencil);
                self.cast(msg);
            }
            self.state.frame = *frame;
        }
    }

    fn bind_shell(&self, meta: &ProgramMeta,
                  (uniforms, blocks, textures): TempProgramResources)
                  -> Result<(), ShellError> {
        self.cast(device::BindProgram(meta.name));
        for (var, value) in meta.uniforms.iter().zip(uniforms.iter()) {
            // unwrap() is safe since the errors were caught in prebind_shell()
            self.cast(device::BindUniform(var.location, value.unwrap()));
        }
        for (i, (var, option)) in meta.blocks.iter().zip(blocks.iter()).enumerate() {
            let BufferHandle(bh) = option.unwrap();
            match self.dispatcher.resource.buffers.get(bh) {
                Ok(&Loaded(block)) =>
                    self.cast(device::BindUniformBlock(
                        meta.name,
                        i as device::UniformBufferSlot,
                        i as device::UniformBlockIndex,
                        block)),
                _ => return Err(ErrorShellBlock(var.name.clone())),
            }
        }
        for (i, (var, option)) in meta.textures.iter().zip(textures.iter()).enumerate() {
            let (TextureHandle(tex_handle), sampler) = option.unwrap();
            let sam = match sampler {
                Some(SamplerHandle(sam)) => match self.dispatcher.resource.samplers[sam] {
                    (Loaded(sam), ref info) => Some((sam, info.clone())),
                    _ => return Err(ErrorShellSampler(var.name.clone())),
                },
                None => None,
            };
            match self.dispatcher.resource.textures.get(tex_handle) {
                Ok(&(Loaded(tex), ref info)) => {
                    self.cast(device::BindUniform(
                        var.location,
                        device::shade::ValueI32(i as i32)
                        ));
                    self.cast(device::BindTexture(
                        i as device::TextureSlot,
                        info.kind,
                        tex,
                        sam));
                },
                _ => return Err(ErrorShellTexture(var.name.clone())),
            }
        }
        Ok(())
    }

    fn bind_mesh(&self, mesh: &mesh::Mesh, prog: &ProgramMeta) -> Result<(), MeshError> {
        for sat in prog.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => match vat.elem_type.is_compatible(sat.base_type) {
                    Ok(_) => {
                        let BufferHandle(buf) = vat.buffer;
                        self.cast(device::BindAttribute(
                            sat.location as device::AttributeSlot,
                            *self.dispatcher.resource.buffers[buf].unwrap(),
                            vat.elem_count, vat.elem_type, vat.stride, vat.offset
                            ))
                    },
                    Err(_) => return Err(ErrorAttributeType)
                },
                None => return Err(ErrorAttributeMissing)
            }
        }
        Ok(())
    }
}
