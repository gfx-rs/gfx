// Copyright 2015 The Gfx-rs Developers.
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

use std::rc::Rc;
use std::{slice, ptr};

use {gl, tex};
use gfx_core::{self as d, factory as f, texture as t, buffer, mapping};
use gfx_core::memory::{Access, Bind, Typed, Usage};
use gfx_core::format::{ChannelType, Format};
use gfx_core::handle::{self, Producer};
use gfx_core::target::{Layer, Level};

use command::CommandBuffer;
use {Resources as R, Share, OutputMerger};
use {Buffer, BufferElement, FatSampler, NewTexture,
     PipelineState, ResourceView, TargetView, Fence};


pub fn role_to_target(role: buffer::Role) -> gl::types::GLenum {
    match role {
        buffer::Role::Vertex   => gl::ARRAY_BUFFER,
        buffer::Role::Index    => gl::ELEMENT_ARRAY_BUFFER,
        buffer::Role::Constant => gl::UNIFORM_BUFFER,
        buffer::Role::Staging  => gl::ARRAY_BUFFER,
    }
}

fn access_to_map_bits(access: Access) -> gl::types::GLenum {
    let mut r = 0;
    if access.contains(Access::READ) { r |= gl::MAP_READ_BIT; }
    if access.contains(Access::WRITE) { r |= gl::MAP_WRITE_BIT; }
    r
}

fn access_to_gl(access: Access) -> gl::types::GLenum {
    match access {
        Access::RW => gl::READ_WRITE,
        Access::READ => gl::READ_ONLY,
        Access::WRITE => gl::WRITE_ONLY,
        _ => unreachable!(),
    }
}

pub fn update_sub_buffer(gl: &gl::Gl, buffer: Buffer, address: *const u8,
                         size: usize, offset: usize, role: buffer::Role) {
    let target = role_to_target(role);
    unsafe {
        gl.BindBuffer(target, buffer);
        gl.BufferSubData(target,
            offset as gl::types::GLintptr,
            size as gl::types::GLsizeiptr,
            address as *const gl::types::GLvoid
        );
    }
}


/// GL resource factory.
pub struct Factory {
    share: Rc<Share>,
}

impl Clone for Factory {
    fn clone(&self) -> Factory {
        Factory::new(self.share.clone())
    }
}

impl Factory {
    /// Create a new `Factory`.
    pub fn new(share: Rc<Share>) -> Factory {
        Factory {
            share: share,
        }
    }

    pub fn create_command_buffer(&mut self) -> CommandBuffer {
        CommandBuffer::new(self.create_fbo_internal(), self.share.workarounds.clone())
    }

    fn create_fbo_internal(&mut self) -> gl::types::GLuint {
        let gl = &self.share.context;
        let mut name = 0 as ::FrameBuffer;
        unsafe {
            gl.GenFramebuffers(1, &mut name);
        }
        info!("\tCreated frame buffer {}", name);
        name
    }

    fn create_buffer_internal(&mut self) -> Buffer {
        let gl = &self.share.context;
        let mut name = 0 as Buffer;
        unsafe { gl.GenBuffers(1, &mut name); }
        info!("\tCreated buffer {}", name);
        name
    }

    fn init_buffer(&mut self,
                   buffer: Buffer,
                   info: &buffer::Info,
                   data_opt: Option<&[u8]>) -> Option<MappingGate> {
        let gl = &self.share.context;
        let target = role_to_target(info.role);
        let mut data_ptr = if let Some(data) = data_opt {
            debug_assert!(data.len() == info.size);
            data.as_ptr() as *const gl::types::GLvoid
        } else {
            0 as *const gl::types::GLvoid
        };

        if self.share.private_caps.buffer_storage_supported {
            let usage = match info.usage {
                Usage::Data => 0,
                // TODO: we could use mapping instead of glBufferSubData
                Usage::Dynamic => gl::DYNAMIC_STORAGE_BIT,
                Usage::Upload => access_to_map_bits(Access::WRITE) | gl::MAP_PERSISTENT_BIT,
                Usage::Download => access_to_map_bits(Access::READ) | gl::MAP_PERSISTENT_BIT,
            };
            let size = if info.size == 0 {
                // we are not allowed to pass size=0 into `glBufferStorage`
                data_ptr = 0 as *const _;
                1
            } else {
                info.size as gl::types::GLsizeiptr
            };
            unsafe {
                gl.BindBuffer(target, buffer);
                gl.BufferStorage(target,
                    size,
                    data_ptr,
                    usage
                );
            }
        }
        else {
            let usage = match info.usage {
                Usage::Data => gl::STATIC_DRAW,
                Usage::Dynamic => gl::DYNAMIC_DRAW,
                Usage::Upload => gl::STREAM_DRAW,
                Usage::Download => gl::STREAM_READ,
            };
            unsafe {
                gl.BindBuffer(target, buffer);
                gl.BufferData(target,
                    info.size as gl::types::GLsizeiptr,
                    data_ptr,
                    usage
                );
            }
        }
        if let Err(err) = self.share.check() {
            panic!("Error {:?} creating buffer: {:?}", err, info)
        }

        let mapping_access = match info.usage {
            Usage::Data | Usage::Dynamic => None,
            Usage::Upload => Some(Access::WRITE),
            Usage::Download => Some(Access::READ),
        };

        mapping_access.map(|access| {
            let (kind, ptr) = if self.share.private_caps.buffer_storage_supported {
                let mut gl_access = access_to_map_bits(access) |
                                    gl::MAP_PERSISTENT_BIT;
                if access.contains(Access::WRITE) {
                    gl_access |= gl::MAP_FLUSH_EXPLICIT_BIT;
                }
                let size = info.size as isize;
                let ptr = unsafe {
                    gl.BindBuffer(target, buffer);
                    gl.MapBufferRange(target, 0, size, gl_access)
                } as *mut ::std::os::raw::c_void;
                (MappingKind::Persistent(mapping::Status::clean()), ptr)
            } else {
                (MappingKind::Temporary, ptr::null_mut())
            };
            if let Err(err) = self.share.check() {
                panic!("Error {:?} mapping buffer: {:?}, with access: {:?}",
                       err, info, access)
            }

            MappingGate {
                kind: kind,
                pointer: ptr,
            }
        })
    }

    fn create_program_raw(&mut self, shader_set: &d::ShaderSet<R>)
                          -> Result<(gl::types::GLuint, d::shade::ProgramInfo), d::shade::CreateProgramError> {
        use shade::create_program;
        let mut handler = handle::Manager::new();
        let mut shaders = [0; 5];
        let usage = shader_set.get_usage();
        let shader_slice = match shader_set {
            &d::ShaderSet::Simple(ref vs, ref ps) => {
                shaders[0] = *vs.reference(&mut handler);
                shaders[1] = *ps.reference(&mut handler);
                &shaders[..2]
            },
            &d::ShaderSet::Geometry(ref vs, ref gs, ref ps) => {
                shaders[0] = *vs.reference(&mut handler);
                shaders[1] = *gs.reference(&mut handler);
                shaders[2] = *ps.reference(&mut handler);
                &shaders[..3]
            },
            &d::ShaderSet::Tessellated(ref vs, ref hs, ref ds, ref ps) => {
                shaders[0] = *vs.reference(&mut handler);
                shaders[1] = *hs.reference(&mut handler);
                shaders[2] = *ds.reference(&mut handler);
                shaders[3] = *ps.reference(&mut handler);
                &shaders[..4]
            },
            &d::ShaderSet::TessellatedGeometry(ref vs, ref hs, ref ds, ref gs, ref ps) => {
                shaders[0] = *vs.reference(&mut handler);
                shaders[1] = *hs.reference(&mut handler);
                shaders[2] = *ds.reference(&mut handler);
                shaders[3] = *gs.reference(&mut handler);
                shaders[4] = *ps.reference(&mut handler);
                &shaders[..5]
            },
        };
        let result = create_program(&self.share.context, &self.share.capabilities,
                                    &self.share.private_caps, shader_slice, usage);
        if let Err(err) = self.share.check() {
            panic!("Error {:?} creating program: {:?}", err, shader_set)
        }
        result
    }

    fn view_texture_as_target(&mut self, htex: &handle::RawTexture<R>, level: Level, layer: Option<Layer>)
                              -> Result<TargetView, f::TargetViewError> {
        match (htex.resource(), layer) {
            (&NewTexture::Surface(_), Some(_)) => Err(f::TargetViewError::Unsupported),
            (&NewTexture::Surface(_), None) if level != 0 => Err(f::TargetViewError::Unsupported),
            (&NewTexture::Surface(s), None) => Ok(TargetView::Surface(s)),
            (&NewTexture::Texture(t), Some(l)) => Ok(TargetView::TextureLayer(t, level, l)),
            (&NewTexture::Texture(t), None) => Ok(TargetView::Texture(t, level)),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MappingKind {
    Persistent(mapping::Status<R>),
    Temporary,
}

#[derive(Debug, Eq, Hash, PartialEq)]
#[allow(missing_copy_implementations)]
pub struct MappingGate {
    pub kind: MappingKind,
    pub pointer: *mut ::std::os::raw::c_void,
}

unsafe impl Send for MappingGate {}
unsafe impl Sync for MappingGate {}

impl mapping::Gate<R> for MappingGate {
    unsafe fn set<T>(&self, index: usize, val: T) {
        *(self.pointer as *mut T).offset(index as isize) = val;
    }

    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T] {
        slice::from_raw_parts(self.pointer as *const T, len)
    }

    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T] {
        slice::from_raw_parts_mut(self.pointer as *mut T, len)
    }
}

pub fn temporary_ensure_mapped(pointer: &mut *mut ::std::os::raw::c_void,
                               target: gl::types::GLenum,
                               buffer: Buffer,
                               access: Access,
                               gl: &gl::Gl) {
    if pointer.is_null() {
        unsafe {
            gl.BindBuffer(target, buffer);
            *pointer = gl.MapBuffer(target, access_to_gl(access))
                as *mut ::std::os::raw::c_void;
        }
    }
}

pub fn temporary_ensure_unmapped(pointer: &mut *mut ::std::os::raw::c_void,
                                 target: gl::types::GLenum,
                                 buffer: Buffer,
                                 gl: &gl::Gl) {
    if !pointer.is_null() {
        unsafe {
            gl.BindBuffer(target, buffer);
            gl.UnmapBuffer(target);
        }

        *pointer = ptr::null_mut();
    }
}

impl f::Factory<R> for Factory {
    fn get_capabilities(&self) -> &d::Capabilities {
        &self.share.capabilities
    }

    fn create_buffer_raw(&mut self, info: buffer::Info) -> Result<handle::RawBuffer<R>, buffer::CreationError> {
        if !self.share.capabilities.constant_buffer_supported && info.role == buffer::Role::Constant {
            error!("Constant buffers are not supported by this GL version");
            return Err(buffer::CreationError::Other);
        }
        let name = self.create_buffer_internal();
        let mapping = self.init_buffer(name, &info, None);
        Ok(self.share.handles.borrow_mut().make_buffer(name, info, mapping))
    }

    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, role: buffer::Role, bind: Bind)
                               -> Result<handle::RawBuffer<R>, buffer::CreationError> {
        let name = self.create_buffer_internal();
        let info = buffer::Info {
            role: role,
            usage: Usage::Data,
            bind: bind,
            size: data.len(),
            stride: stride,
        };
        let mapping = self.init_buffer(name, &info, Some(data));
        Ok(self.share.handles.borrow_mut().make_buffer(name, info, mapping))
    }

    fn create_shader(&mut self, stage: d::shade::Stage, code: &[u8])
                     -> Result<handle::Shader<R>, d::shade::CreateShaderError> {
        ::shade::create_shader(&self.share.context, stage, code)
                .map(|sh| self.share.handles.borrow_mut().make_shader(sh))
    }

    fn create_program(&mut self, shader_set: &d::ShaderSet<R>)
                      -> Result<handle::Program<R>, d::shade::CreateProgramError> {
        self.create_program_raw(shader_set)
            .map(|(name, info)| self.share.handles.borrow_mut().make_program(name, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &handle::Program<R>, desc: &d::pso::Descriptor)
                                 -> Result<handle::RawPipelineState<R>, d::pso::CreationError> {
        use gfx_core::state as s;
        let caps = &self.share.capabilities;
        match desc.primitive {
            d::Primitive::PatchList(num) if num == 0 || (num as usize) > caps.max_patch_size =>
                return Err(d::pso::CreationError),
            _ => ()
        }
        let color = s::Color {
            mask: s::ColorMask::all(),
            blend: None,
        };
        let mut output = OutputMerger {
            draw_mask: 0,
            stencil: match desc.depth_stencil {
                Some((_, t)) if t.front.is_some() || t.back.is_some() => Some(s::Stencil {
                    front: t.front.unwrap_or_default(),
                    back: t.back.unwrap_or_default(),
                }),
                _ => None,
            },
            depth: desc.depth_stencil.and_then(|(_, t)| t.depth),
            colors: [color; d::MAX_COLOR_TARGETS],
        };
        for i in 0 .. d::MAX_COLOR_TARGETS {
            if let Some((_, ref bi)) = desc.color_targets[i] {
                output.draw_mask |= 1<<i;
                output.colors[i].mask = bi.mask;
                if bi.color.is_some() || bi.alpha.is_some() {
                    output.colors[i].blend = Some(s::Blend {
                        color: bi.color.unwrap_or_default(),
                        alpha: bi.alpha.unwrap_or_default(),
                    });
                }
            }
        }
        let mut inputs = [None; d::MAX_VERTEX_ATTRIBUTES];
        for i in 0 .. d::MAX_VERTEX_ATTRIBUTES {
            inputs[i] = desc.attributes[i].map(|at| BufferElement {
                // Note: this `Option::unwrap` call fails the emscripten (sdk-1.38.16-64bit) build. Inlining the call seems to work around the issue. (See #2721)
                #[cfg(target_os = "emscripten")]
                desc: match desc.vertex_buffers[at.0 as usize] {
                    Some(val) => val,
                    None => panic!("called `Option::unwrap()` on a `None` value"),
                },
                #[cfg(not(target_os = "emscripten"))]
                desc: desc.vertex_buffers[at.0 as usize].unwrap(),
                elem: at.1,
            });
        }
        let pso = PipelineState {
            program: *program.resource(),
            primitive: desc.primitive,
            input: inputs,
            scissor: desc.scissor,
            rasterizer: desc.rasterizer,
            output: output,
        };
        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
    }

    fn create_texture_raw(&mut self, desc: t::Info, hint: Option<ChannelType>, data_opt: Option<(&[&[u8]], t::Mipmap)>)
                          -> Result<handle::RawTexture<R>, t::CreationError> {
        use gfx_core::texture::CreationError;
        let caps = &self.share.private_caps;
        if desc.levels == 0 {
            return Err(CreationError::Size(0))
        }
        let dim = desc.kind.get_dimensions();
        let max_size = self.share.capabilities.max_texture_size;
        if dim.0 as usize > max_size {
            return Err(CreationError::Size(dim.0));
        }
        if dim.1 as usize > max_size {
            return Err(CreationError::Size(dim.1));
        }
        let cty = hint.unwrap_or(ChannelType::Uint); //careful here
        let gl = &self.share.context;
        let object = if desc.bind.intersects(Bind::SHADER_RESOURCE | Bind::UNORDERED_ACCESS) || data_opt.is_some() {
            let name = if caps.immutable_storage_supported {
                try!(tex::make_with_storage(gl, &desc, cty))
            } else {
                try!(tex::make_without_storage(gl, &desc, cty))
            };
            if let Some(data) = data_opt {
                try!(tex::init_texture_data(gl, name, desc, cty, data));
            }
            NewTexture::Texture(name)
        }else {
            let name = try!(tex::make_surface(gl, &desc, cty));
            NewTexture::Surface(name)
        };
        if let Err(err) = self.share.check() {
            panic!("Error {:?} creating texture: {:?}, hint: {:?}", err, desc, hint)
        }
        Ok(self.share.handles.borrow_mut().make_texture(object, desc))
    }

    fn view_buffer_as_shader_resource_raw(&mut self, hbuf: &handle::RawBuffer<R>, format: Format)
                                      -> Result<handle::RawShaderResourceView<R>, f::ResourceViewError> {
        let gl = &self.share.context;
        let mut name = 0 as gl::types::GLuint;
        let buf_name = *hbuf.resource();
        let format = tex::format_to_glfull(format)
            .map_err(|_| f::ResourceViewError::Unsupported)?;
        unsafe {
            gl.GenTextures(1, &mut name);
            gl.BindTexture(gl::TEXTURE_BUFFER, name);
            gl.TexBuffer(gl::TEXTURE_BUFFER, format, buf_name);
        }
        let view = ResourceView::new_buffer(name);
        if let Err(err) = self.share.check() {
            panic!("Error {:?} creating buffer SRV: {:?}", err, hbuf.get_info());
        }
        Ok(self.share.handles.borrow_mut().make_buffer_srv(view, hbuf))
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &handle::RawBuffer<R>)
                                       -> Result<handle::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &handle::RawTexture<R>, desc: t::ResourceDesc)
                                           -> Result<handle::RawShaderResourceView<R>, f::ResourceViewError> {
        match htex.resource() {
            &NewTexture::Surface(_) => Err(f::ResourceViewError::NoBindFlag),
            &NewTexture::Texture(source) => {
                //TODO: use the view descriptor
                let info = htex.get_info();
                let view = if desc.layer.is_none() && desc.min == 0 && desc.max + 1 >= info.levels {
                    ResourceView::new_texture(source, info.kind)
                } else if self.share.private_caps.texture_view_supported {
                    let t = tex::make_view(&self.share.context, source, &info, &desc)?;
                    ResourceView::new_texture_owned(t, info.kind)
                } else {
                    error!("SRV descriptor doesn't cover the whole resource, and views are not supported by this GL context: {:?}",
                        desc);
                    return Err(f::ResourceViewError::Unsupported);
                };
                if let Err(err) = self.share.check() {
                    panic!("Error {:?} creating SRV: {:?}, desc: {:?}", err, info, desc);
                }
                Ok(self.share.handles.borrow_mut().make_texture_srv(view, htex))
            },
        }
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &handle::RawTexture<R>)
                                        -> Result<handle::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &handle::RawTexture<R>, desc: t::RenderDesc)
                                         -> Result<handle::RawRenderTargetView<R>, f::TargetViewError> {
        self.view_texture_as_target(htex, desc.level, desc.layer)
            .map(|view| {
                let dim = htex.get_info().kind.get_level_dimensions(desc.level);
                self.share.handles.borrow_mut().make_rtv(view, htex, dim)
            })
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &handle::RawTexture<R>, desc: t::DepthStencilDesc)
                                         -> Result<handle::RawDepthStencilView<R>, f::TargetViewError> {
        self.view_texture_as_target(htex, desc.level, desc.layer)
            .map(|view| {
                let dim = htex.get_info().kind.get_level_dimensions(0);
                self.share.handles.borrow_mut().make_dsv(view, htex, dim)
            })
    }

    fn create_sampler(&mut self, info: t::SamplerInfo) -> handle::Sampler<R> {
        let name = if self.share.private_caps.sampler_objects_supported {
            tex::make_sampler(&self.share.context, &info, &self.share.private_caps)
        } else {
            0
        };
        let sam = FatSampler {
            object: name,
            info: info.clone(),
        };
        if let Err(err) = self.share.check() {
            panic!("Error {:?} creating sampler: {:?}", err, info)
        }
        self.share.handles.borrow_mut().make_sampler(sam, info)
    }

    fn read_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<R, T>)
                               -> Result<mapping::Reader<'b, R, T>,
                                         mapping::Error>
        where T: Copy
    {
        let gl = &self.share.context;
        unsafe {
            mapping::read(buf.raw(), |mapping| match mapping.kind {
                MappingKind::Persistent(ref mut status) =>
                    status.cpu_access(|fence| wait_fence(fence.resource(), gl)),
                MappingKind::Temporary =>
                    temporary_ensure_mapped(&mut mapping.pointer,
                                            role_to_target(buf.get_info().role),
                                            *buf.raw().resource(),
                                            Access::READ,
                                            gl),
            })
        }
    }

    fn write_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<R, T>)
                                -> Result<mapping::Writer<'b, R, T>,
                                          mapping::Error>
        where T: Copy
    {
        let gl = &self.share.context;
        unsafe {
            mapping::write(buf.raw(), |mapping| match mapping.kind {
                MappingKind::Persistent(ref mut status) =>
                    status.cpu_write_access(|fence| wait_fence(fence.resource(), gl)),
                MappingKind::Temporary =>
                    temporary_ensure_mapped(&mut mapping.pointer,
                                            role_to_target(buf.get_info().role),
                                            *buf.raw().resource(),
                                            Access::WRITE,
                                            gl),
            })
        }
    }
}

pub fn wait_fence(fence: &Fence, gl: &gl::Gl) {
    let timeout = 1_000_000_000_000;
    // TODO: use the return value of this call
    // TODO:
    // This can be called by multiple objects wanting to ensure they have exclusive
    // access to a resource. How much does this call costs ? The status of the fence
    // could be cached to avoid calling this more than once (in core or in the backend ?).
    unsafe {
        gl.ClientWaitSync(fence.0, gl::SYNC_FLUSH_COMMANDS_BIT, timeout);
    }
}
