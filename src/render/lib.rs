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
extern crate comm;
extern crate device;

use std::fmt::Show;
use std::mem::size_of;
use std::vec::MoveItems;

use backend = device::dev;
use device::shade::{CreateShaderError, ProgramMeta, Vertex, Fragment, ShaderSource};
use device::target::{ClearData, Target, TargetColor, TargetDepth, TargetStencil};
use shade::{BundleInternal, ShaderParam};
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

pub mod mesh;
pub mod resource;
pub mod shade;
pub mod state;
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
#[deriving(Show)]
pub enum BundleError {
    /// Error from a uniform block.
    ErrorBundleBlock(shade::VarBlock),
    /// Error from a texture.
    ErrorBundleTexture(shade::VarTexture),
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
pub enum DrawError<'a> {
    /// Error with a program.
    ErrorProgram,
    /// Error with the program bundle.
    ErrorBundle(BundleError),
    /// Error with the mesh.
    ErrorMesh(MeshError),
	/// Error with the mesh slice
    ErrorSlice,
}

struct Dispatcher {
    /// Channel to receive device messages
    channel: Receiver<device::Reply<Token>>,
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
            let reply = self.channel.recv();
            match self.resource.process(reply) {
                Ok(_) => (),
                Err(e) => self.errors.push(e),
            }
        }
    }

    /// Get a guaranteed copy of a specific resource accessed by the function.
    fn get_any<R: Copy, E: Show>(&mut self, fun: <'a>|&'a resource::Cache| -> &'a resource::Future<R, E>) -> R {
        self.demand(|res| !fun(res).is_pending());
        *fun(&self.resource).unwrap()
    }

    fn get_buffer(&mut self, handle: BufferHandle) -> backend::Buffer {
        let BufferHandle(h) = handle;
        self.get_any(|res| &res.buffers[h])
    }

    fn get_shader(&mut self, handle: ShaderHandle) -> backend::Shader {
        let ShaderHandle(h) = handle;
        self.get_any(|res| &res.shaders[h])
    }
}

/// A renderer. Methods on this get translated into commands for the device.
pub struct Renderer {
    dispatcher: Dispatcher,
    device_tx: Sender<device::Request<Token>>,
    swap_ack: Receiver<device::Ack>,
    should_finish: comm::ShouldClose,
    /// the shared VAO and FBO
    common_array_buffer: Token,
    common_frame_buffer: Token,
    /// the default FBO for drawing
    default_frame_buffer: backend::FrameBuffer,
    /// current state
    state: State,
}

impl Renderer {
    /// Create a new `Renderer` using given channels for communicating with the device. Generally,
    /// you want to use `gfx::start` instead.
    pub fn new(device_tx: Sender<device::Request<Token>>, device_rx: Receiver<device::Reply<Token>>,
            swap_rx: Receiver<device::Ack>, should_finish: comm::ShouldClose) -> Renderer {
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
                errors: Vec::new(),
                resource: res,
            },
            device_tx: device_tx,
            swap_ack: swap_rx,
            should_finish: should_finish,
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
        self.should_finish.check()
    }

    /// Iterate over any errors that have been raised by the device when trying to issue commands
    /// since the last time this method was called.
    pub fn errors(&mut self) -> MoveItems<DeviceError> {
        let errors = self.dispatcher.errors.clone();
        self.dispatcher.errors.clear();
        errors.move_iter()
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ClearData, frame: target::Frame) {
        self.bind_frame(&frame);
        self.cast(device::Clear(data));
    }

    /// Draw `slice` of `mesh` into `frame`, using a `bundle` of shader program and parameters, and
    /// a given draw state.
    pub fn draw<'a, L, T: shade::ShaderParam<L>>(&'a mut self, mesh: &mesh::Mesh, slice: mesh::Slice, frame: target::Frame,
            bundle: &shade::ShaderBundle<L, T>, state: state::DrawState) -> Result<(), DrawError<'a>> {
        // demand resources. This section needs the mutable self, so we are unable to do this
        // after we get a reference to ether the `Environment` or the `ProgramMeta`
        self.prebind_mesh(mesh, &slice);
        self.prebind_bundle(bundle);
        // bind state
        self.cast(device::SetPrimitiveState(state.primitive));
        self.cast(device::SetScissor(state.scissor));
        self.cast(device::SetDepthStencilState(state.depth, state.stencil,
            state.primitive.get_cull_mode()));
        self.cast(device::SetBlendState(state.blend));
        self.cast(device::SetColorMask(state.color_mask));
        // bind array buffer
        let h_vao = self.common_array_buffer;
        let vao = self.dispatcher.get_any(|res| &res.array_buffers[h_vao]);
        self.cast(device::BindArrayBuffer(vao));
        // bind output frame
        self.bind_frame(&frame);
        // bind shaders
        let ProgramHandle(ph) = bundle.get_program();
        let program = match self.dispatcher.resource.programs.get(ph) {
            Ok(&resource::Pending) => fail!("Program is not loaded yet"),
            Ok(&resource::Loaded(ref p)) => p,
            _ => return Err(ErrorProgram),
        };
        match self.bind_shader_bundle(program, bundle) {
            Ok(_) => (),
            Err(e) => return Err(ErrorBundle(e)),
        }
        // bind vertex attributes
        match self.bind_mesh(mesh, program) {
            Ok(_) => (),
            Err(e) => return Err(ErrorMesh(e)),
        }
        // draw
        match slice {
            mesh::VertexSlice(start, end) => {
                self.cast(device::Draw(start, end));
            },
            mesh::IndexSlice(handle, start, end) => {
                let BufferHandle(bh) = handle;
                let buf = match self.dispatcher.resource.buffers.get(bh) {
                    Ok(&Loaded(buf)) => buf,
                    _ => return Err(ErrorSlice),
                };
                self.cast(device::BindIndex(buf));
                self.cast(device::DrawIndexed(start, end));
            },
        }
        Ok(())
    }

    /// Finish rendering a frame. Waits for a frame to be finished drawing, as specified by the
    /// queue size passed to `gfx::start`.
    pub fn end_frame(&self) {
        self.device_tx.send(device::SwapBuffers);
        self.swap_ack.recv();  //wait for acknowlegement
    }

    /// --- Resource creation --- ///

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

    /// Create a new mesh from the given vertex data.
    ///
    /// Convenience function around `crate_buffer` and `Mesh::from`.
    pub fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh {
        let nv = data.len();
        debug_assert!(nv >> (8 * size_of::<mesh::VertexCount>()) == 0);
        let buf = self.create_buffer(Some(data));
        mesh::Mesh::from::<T>(buf, nv as mesh::VertexCount)
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

    /// --- Resource deletion --- ///

    pub fn delete_program(&mut self, handle: ProgramHandle) {
        let ProgramHandle(h) = handle;
        let v = self.dispatcher.resource.programs.remove(h).unwrap().unwrap().name;
        self.cast(device::DeleteProgram(v));
    }

    pub fn delete_buffer(&mut self, handle: BufferHandle) {
        let BufferHandle(h) = handle;
        let v = *self.dispatcher.resource.buffers.remove(h).unwrap().unwrap();
        self.cast(device::DeleteBuffer(v));
    }

    pub fn delete_surface(&mut self, handle: SurfaceHandle) {
        let SurfaceHandle(h) = handle;
        let v = *self.dispatcher.resource.surfaces.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteSurface(v));
    }

    pub fn delete_texture(&mut self, handle: TextureHandle) {
        let TextureHandle(h) = handle;
        let v = *self.dispatcher.resource.textures.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteTexture(v));
    }

    pub fn delete_sampler(&mut self, handle: SamplerHandle) {
        let SamplerHandle(h) = handle;
        let v = *self.dispatcher.resource.samplers.remove(h).unwrap().ref0().unwrap();
        self.cast(device::DeleteSampler(v));
    }

    /// --- Resource modification --- ///

    /// Bundle together a program with its parameters.
    pub fn bundle_program<'a, L, T: shade::ShaderParam<L>>(&'a mut self, prog: ProgramHandle, data: T)
             -> Result<shade::ShaderBundle<L, T>, shade::ParameterLinkError<'a>> {
        let ProgramHandle(ph) = program;
        self.dispatcher.demand(|res| !res.programs[ph].is_pending());
        match self.dispatcher.resource.programs.get(ph) {
            Ok(&Loaded(ref m)) => {
                let mut sink = shade::MetaSink::new(m.clone());
                match data.create_link(&mut sink) {
                    Ok(link) => match sink.complete() {
                        Ok(_) => Ok(BundleInternal::new(
                            None::<&shade::ShaderBundle<L, T>>, // a workaround to specify the type
                            program, data, link)),
                        Err(e) => Err(shade::ErrorMissingParameter(e)),
                    },
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
        let name = self.dispatcher.get_any(|res| res.textures[tex].ref0());
        let info = self.dispatcher.resource.textures[tex].ref1();
        self.cast(device::UpdateTexture(info.kind, name, img, (box data) as Box<device::Blob + Send>));
    }

    /// --- Resource binding --- ///

    /// Make sure all the mesh buffers are successfully created/loaded
    fn prebind_mesh(&mut self, mesh: &mesh::Mesh, slice: &mesh::Slice) {
        for at in mesh.attributes.iter() {
            self.dispatcher.get_buffer(at.buffer);
        }
        match *slice {
            mesh::IndexSlice(handle, _, _) =>
                self.dispatcher.get_buffer(handle),
            _ => 0,
        };
    }

    fn prebind_bundle<L, T: shade::ShaderParam<L>>(&mut self, bundle: &shade::ShaderBundle<L, T>) {
        let dp = &mut self.dispatcher;
        // buffers pass
        bundle.bind(|_, _| {
        }, |_, BufferHandle(buf)| {
            dp.demand(|res| !res.buffers[buf].is_pending());
        }, |_, _| {

        });
        // texture pass
        bundle.bind(|_, _| {
        }, |_, _| {
        }, |_, (TextureHandle(tex), sampler)| {
            dp.demand(|res| !res.textures[tex].ref0().is_pending());
            match sampler {
                Some(SamplerHandle(sam)) =>
                    dp.demand(|res| !res.samplers[sam].ref0().is_pending()),
                None => (),
            }
        });
    }

    fn make_target_cast(dp: &mut Dispatcher, to: Target, plane: target::Plane) -> device::CastRequest {
        match plane {
            target::PlaneEmpty => device::UnbindTarget(to),
            target::PlaneSurface(SurfaceHandle(suf)) => {
                let name = dp.get_any(|res| res.surfaces[suf].ref0());
                device::BindTargetSurface(to, name)
            },
            target::PlaneTexture(TextureHandle(tex), level, layer) => {
                let name = dp.get_any(|res| res.textures[tex].ref0());
                device::BindTargetTexture(to, name, level, layer)
            },
        }
    }

    fn bind_frame(&mut self, frame: &target::Frame) {
        self.cast(device::SetViewport(device::target::Rect {
            x: 0,
            y: 0,
            w: frame.size[0],
            h: frame.size[1],
        }));
        if frame.is_default() {
            // binding the default FBO, not touching our common one
            self.cast(device::BindFrameBuffer(self.default_frame_buffer));
        } else {
            let h_fbo = self.common_frame_buffer;
            let fbo = self.dispatcher.get_any(|res| &res.frame_buffers[h_fbo]);
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

    fn bind_shader_bundle<L, T: shade::ShaderParam<L>>(&self, meta: &ProgramMeta,
            bundle: &shade::ShaderBundle<L, T>) -> Result<(), BundleError> {
        self.cast(device::BindProgram(meta.name));
        let mut block_slot   = 0u as device::UniformBufferSlot;
        let mut texture_slot = 0u as device::TextureSlot;
        let mut block_fail   = None::<shade::VarBlock>;
        let mut texture_fail = None::<shade::VarTexture>;
        bundle.bind(|uv, value| {
            self.cast(device::BindUniform(meta.uniforms[uv as uint].location, value));
        }, |bv, BufferHandle(bh)| {
            match self.dispatcher.resource.buffers.get(bh) {
                Ok(&Loaded(block)) => {
                    self.cast(device::BindUniformBlock(meta.name,
                        block_slot as device::UniformBufferSlot,
                        bv as device::UniformBlockIndex,
                        block));
                    block_slot += 1;
                },
                _ => {block_fail = Some(bv)},
            }
        }, |tv, (TextureHandle(tex_handle), sampler)| {
            let sam = sampler.map(|SamplerHandle(sam)|
                match self.dispatcher.resource.samplers[sam] {
                    (ref future, ref info) => (*future.unwrap(), info.clone())
                }
            );
            match self.dispatcher.resource.textures.get(tex_handle) {
                Ok(&(Loaded(tex), ref info)) => {
                    self.cast(device::BindUniform(
                        meta.textures[tv as uint].location,
                        device::shade::ValueI32(texture_slot as i32)
                        ));
                    self.cast(device::BindTexture(texture_slot, info.kind, tex, sam));
                    texture_slot += 1;
                },
                _ => {texture_fail = Some(tv)},
            }
        });
        match (block_fail, texture_fail) {
            (Some(bv), _) => Err(ErrorBundleBlock(bv)),
            (_, Some(tv)) => Err(ErrorBundleTexture(tv)),
            (None, None)  => Ok(()),
        }
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
