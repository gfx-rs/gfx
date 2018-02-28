use std::{mem, ptr, slice};
use std::borrow::{Borrow, BorrowMut};
use Starc;

use hal;
use hal::error;

use gl;
use smallvec::SmallVec;

use {command as com, native, state, window};
use info::LegacyFeatures;
use {Backend, Share};

pub type ArrayBuffer = gl::types::GLuint;

// State caching system for command queue.
//
// We track the current global state, which is based on
// the restriction that we only expose _one_ command queue.
//
// This allows us to minimize additional driver calls to
// ensure that command buffers are handled isolated of each other.
struct State {
    // Indicate if the vertex array object is bound.
    // If VAOs are not supported, this will be also set to true.
    vao: bool,
    // Currently bound index/element buffer.
    // None denotes that we don't know what is currently bound.
    index_buffer: Option<gl::types::GLuint>,
    // Currently set viewports.
    num_viewports: usize,
    // Currently set scissor rects.
    num_scissors: usize,
}

impl State {
    // Create a new state, representing the initial context state
    // as exposed by OpenGL.
    fn new() -> Self {
        State {
            vao: false,
            index_buffer: None,
            num_viewports: 0,
            num_scissors: 0,
        }
    }

    // Invalidate the current state, forcing a complete reset.
    // Required if we allow users to manually inject OpenGL calls.
    fn flush(&mut self) {
        self.vao = false;
        self.index_buffer = None;

        // TOOD: reset viewports and scissors
        //       do we need to clear everything from 0..MAX_VIEWPORTS?
    }
}

pub struct CommandQueue {
    pub(crate) share: Starc<Share>,
    vao: ArrayBuffer,
    state: State,
}

impl CommandQueue {
    /// Create a new command queue.
    pub(crate) fn new(share: &Starc<Share>, vao: ArrayBuffer) -> Self {
        CommandQueue {
            share: share.clone(),
            vao,
            state: State::new(),
        }
    }

    /// Access the OpenGL directly via a closure. OpenGL types and enumerations
    /// can be found in the `gl` crate.
    ///
    /// > Note: Calling this function can have a noticeable impact on the performance
    ///         because the internal state cache will flushed.
    pub unsafe fn with_gl<F: FnMut(&gl::Gl)>(&mut self, mut fun: F) {
        self.reset_state();
        fun(&self.share.context);
        // Flush the state to enforce a reset once a new command buffer
        // is execute because we have no control of the called functions.
        self.state.flush();
    }

    /*
    fn bind_attribute(&mut self, slot: hal::AttributeSlot, buffer: n::Buffer, bel: BufferElement) {
        use core::format::SurfaceType as S;
        use core::format::ChannelType as C;
        let (fm8, fm16, fm32) = match bel.elem.format.1 {
            C::Int | C::Inorm =>
                (gl::BYTE, gl::SHORT, gl::INT),
            C::Uint | C::Unorm =>
                (gl::UNSIGNED_BYTE, gl::UNSIGNED_SHORT, gl::UNSIGNED_INT),
            C::Float => (gl::ZERO, gl::HALF_FLOAT, gl::FLOAT),
            C::Srgb => {
                error!("Unsupported Srgb channel type");
                return
            }
        };
        let (count, gl_type) = match bel.elem.format.0 {
            S::R8              => (1, fm8),
            S::R8_G8           => (2, fm8),
            S::R8_G8_B8_A8     => (4, fm8),
            S::R16             => (1, fm16),
            S::R16_G16         => (2, fm16),
            S::R16_G16_B16     => (3, fm16),
            S::R16_G16_B16_A16 => (4, fm16),
            S::R32             => (1, fm32),
            S::R32_G32         => (2, fm32),
            S::R32_G32_B32     => (3, fm32),
            S::R32_G32_B32_A32 => (4, fm32),
            _ => {
                error!("Unsupported element type: {:?}", bel.elem.format.0);
                return
            }
        };
        let gl = &self.share.context;
        unsafe { gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
        let offset = bel.elem.offset as *const gl::types::GLvoid;
        let stride = bel.desc.stride as gl::types::GLint;
        match bel.elem.format.1 {
            C::Int | C::Uint => unsafe {
                gl.VertexAttribIPointer(slot as gl::types::GLuint,
                    count, gl_type, stride, offset);
            },
            C::Inorm | C::Unorm => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::TRUE, stride, offset);
            },
            //C::Iscaled | C::Uscaled => unsafe {
            //    gl.VertexAttribPointer(slot as gl::types::GLuint,
            //        count, gl_type, gl::FALSE, stride, offset);
            //},
            C::Float => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::FALSE, stride, offset);
            },
            C::Srgb => (),
        }
        unsafe { gl.EnableVertexAttribArray(slot as gl::types::GLuint) };
        if self.share.capabilities.instance_rate {
            unsafe { gl.VertexAttribDivisor(slot as gl::types::GLuint,
                bel.desc.rate as gl::types::GLuint) };
        } else if bel.desc.rate != 0 {
            error!("Instanced arrays are not supported");
        }
    }
    */

    fn bind_target(&mut self, point: gl::types::GLenum, attachment: gl::types::GLenum, view: &native::ImageView) {
        let gl = &self.share.context;
        match view {
            &native::ImageView::Surface(surface) => unsafe {
                gl.FramebufferRenderbuffer(point, attachment, gl::RENDERBUFFER, surface);
            },
            &native::ImageView::Texture(texture, level) => unsafe {
                gl.FramebufferTexture(point, attachment, texture,
                                      level as gl::types::GLint);
            },
            &native::ImageView::TextureLayer(texture, level, layer) => unsafe {
                gl.FramebufferTextureLayer(point, attachment, texture,
                                           level as gl::types::GLint,
                                           layer as gl::types::GLint);
            },
        }
    }

    fn _unbind_target(&mut self, point: gl::types::GLenum, attachment: gl::types::GLenum) {
        let gl = &self.share.context;
        unsafe { gl.FramebufferTexture(point, attachment, 0, 0) };
    }

    /// Return a reference to a stored data object.
    fn get<T>(data: &[u8], ptr: com::BufferSlice) -> &[T] {
        let u32_size = mem::size_of::<T>();
        assert_eq!(ptr.size % u32_size as u32, 0);
        let raw = Self::get_raw(data, ptr);
        unsafe {
            slice::from_raw_parts(
                raw.as_ptr() as *const _,
                raw.len() / u32_size,
            )
        }
    }

    /// Return a reference to a stored data object.
    fn get_raw(data: &[u8], ptr: com::BufferSlice) -> &[u8] {
        assert!(data.len() >= (ptr.offset + ptr.size) as usize);
        &data[ptr.offset as usize..(ptr.offset + ptr.size) as usize]
    }

    // Reset the state to match our _expected_ state before executing
    // a command buffer.
    fn reset_state(&mut self) {
        let gl = &self.share.context;
        let priv_caps = &self.share.private_caps;

        // Bind default VAO
        if !self.state.vao {
            if priv_caps.vertex_array {
                unsafe { gl.BindVertexArray(self.vao) };
            }
            self.state.vao = true
        }

        // Reset indirect draw buffer
        if self.share.legacy_features.contains(LegacyFeatures::INDIRECT_EXECUTION) {
            unsafe { gl.BindBuffer(gl::DRAW_INDIRECT_BUFFER, 0) };
        }

        // Unbind index buffers
        match self.state.index_buffer {
            Some(0) => (), // Nothing to do
            Some(_) | None => {
                unsafe { gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0) };
                self.state.index_buffer = Some(0);
            }
        }

        // Reset viewports
        if self.state.num_viewports == 1 {
            unsafe { gl.Viewport(0, 0, 0, 0) };
            unsafe { gl.DepthRange(0.0, 1.0) };
        } else if self.state.num_viewports > 1 {
            // 16 viewports is a common limit set in drivers.
            let viewports: SmallVec<[[f32; 4]; 16]> =
                (0..self.state.num_viewports)
                    .map(|_| [0.0, 0.0, 0.0, 0.0])
                    .collect();
            let depth_ranges: SmallVec<[[f64; 2]; 16]> =
                (0..self.state.num_viewports)
                    .map(|_| [0.0, 0.0])
                    .collect();
            unsafe { gl.ViewportArrayv(0, viewports.len() as i32, viewports.as_ptr() as *const _)};
            unsafe { gl.DepthRangeArrayv(0, depth_ranges.len() as i32, depth_ranges.as_ptr() as *const _)};
        }

        // Reset scissors
        if self.state.num_scissors == 1 {
            unsafe { gl.Scissor(0, 0, 0, 0) };
        } else if self.state.num_scissors > 1 {
            // 16 viewports is a common limit set in drivers.
            let scissors: SmallVec<[[i32; 4]; 16]> =
                (0..self.state.num_scissors)
                    .map(|_| [0, 0, 0, 0])
                    .collect();
            unsafe { gl.ScissorArrayv(0, scissors.len() as i32, scissors.as_ptr() as *const _)};
        }
    }

    fn process(&mut self, cmd: &com::Command, data_buf: &[u8]) {
        match *cmd {
            com::Command::BindIndexBuffer(buffer) => {
                let gl = &self.share.context;
                self.state.index_buffer = Some(buffer);
                unsafe { gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buffer) };
            }
//          com::Command::BindVertexBuffers(_data_ptr) =>
            com::Command::Draw { primitive, ref vertices, ref instances } => {
                let gl = &self.share.context;
                let legacy = &self.share.legacy_features;
                if instances == &(0u32..1) {
                    unsafe {
                        gl.DrawArrays(
                            primitive,
                            vertices.start as _,
                            (vertices.end - vertices.start) as _,
                        );
                    }
                } else if legacy.contains(LegacyFeatures::DRAW_INSTANCED) {
                    if instances.start == 0 {
                        unsafe {
                            gl.DrawArraysInstanced(
                                primitive,
                                vertices.start as _,
                                (vertices.end - vertices.start) as _,
                                instances.end as _,
                            );
                        }
                    } else if legacy.contains(LegacyFeatures::DRAW_INSTANCED_BASE) {
                        unsafe {
                            gl.DrawArraysInstancedBaseInstance(
                                primitive,
                                vertices.start as _,
                                (vertices.end - vertices.start) as _,
                                (instances.end - instances.start) as _,
                                instances.start as _,
                            );
                        }
                    } else {
                        error!("Instanced draw calls with non-zero base instance are not supported");
                    }
                } else {
                    error!("Instanced draw calls are not supported");
                }
            }
            com::Command::DrawIndexed { primitive, index_type, index_count, index_buffer_offset, base_vertex, ref instances } => {
                let gl = &self.share.context;
                let legacy = &self.share.legacy_features;
                let offset = index_buffer_offset as *const gl::types::GLvoid;

                if instances == &(0u32..1) {
                    if base_vertex == 0 {
                        unsafe {
                            gl.DrawElements(
                                primitive,
                                index_count as _,
                                index_type,
                                offset,
                            );
                        }
                    } else if legacy.contains(LegacyFeatures::DRAW_INDEXED_BASE) {
                        unsafe {
                            gl.DrawElementsBaseVertex(
                                primitive,
                                index_count as _,
                                index_type,
                                offset,
                                base_vertex as _,
                            );
                        }
                    } else {
                        error!("Base vertex with indexed drawing not supported");
                    }
                } else if legacy.contains(LegacyFeatures::DRAW_INDEXED_INSTANCED) {
                    if base_vertex == 0 && instances.start == 0 {
                        unsafe {
                            gl.DrawElementsInstanced(
                                primitive,
                                index_count as _,
                                index_type,
                                offset,
                                instances.end as _,
                            );
                        }
                    } else if instances.start == 0 && legacy.contains(LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE_VERTEX) {
                        unsafe {
                            gl.DrawElementsInstancedBaseVertex(
                                primitive,
                                index_count as _,
                                index_type,
                                offset,
                                instances.end as _,
                                base_vertex as _,
                            );
                        }
                    } else if instances.start == 0 {
                        error!("Base vertex with instanced indexed drawing is not supported");
                    } else if legacy.contains(LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE) {
                        unsafe {
                            gl.DrawElementsInstancedBaseVertexBaseInstance(
                                primitive,
                                index_count as _,
                                index_type,
                                offset,
                                (instances.end - instances.start) as _,
                                base_vertex as _,
                                instances.start as _,
                            );
                        }
                    } else {
                        error!("Instance bases with instanced indexed drawing is not supported");
                    }
                } else {
                    error!("Instanced indexed drawing is not supported");
                }
            }
            com::Command::Dispatch(count) => {
                // Capability support is given by which queue types will be exposed.
                // If there is no compute support, this pattern should never be reached
                // because no queue with compute capability can be created.
                let gl = &self.share.context;
                unsafe { gl.DispatchCompute(count[0], count[1], count[2]) };
            }
            com::Command::DispatchIndirect(buffer, offset) => {
                // Capability support is given by which queue types will be exposed.
                // If there is no compute support, this pattern should never be reached
                // because no queue with compute capability can be created.
                let gl = &self.share.context;
                unsafe {
                    gl.BindBuffer(gl::DRAW_INDIRECT_BUFFER, buffer);
                    // TODO: possible integer conversion issue
                    gl.DispatchComputeIndirect(offset as _);
                }
            }
            com::Command::SetViewports { viewport_ptr, depth_range_ptr } => {
                let gl = &self.share.context;
                let viewports = Self::get::<[f32; 4]>(data_buf, viewport_ptr);
                let depth_ranges = Self::get::<[f64; 2]>(data_buf, depth_range_ptr);

                let num_viewports = viewports.len();
                assert_eq!(num_viewports, depth_ranges.len());
                assert!(0 < num_viewports && num_viewports <= self.share.limits.max_viewports);

                if num_viewports == 1 {
                    let view = viewports[0];
                    let depth_range  = depth_ranges[0];
                    unsafe { gl.Viewport(view[0] as i32, view[1] as i32, view[2] as i32, view[3] as i32) };
                    unsafe { gl.DepthRange(depth_range[0], depth_range[1]) };
                } else if num_viewports > 1 {
                    // Support for these functions is coupled with the support
                    // of multiple viewports.
                    unsafe { gl.ViewportArrayv(0, num_viewports as i32, viewports.as_ptr() as *const _) };
                    unsafe { gl.DepthRangeArrayv(0, num_viewports as i32, depth_ranges.as_ptr() as *const _) };
                }
            }
            com::Command::SetScissors(data_ptr) => {
                let gl = &self.share.context;
                let scissors = Self::get::<[i32; 4]>(data_buf, data_ptr);
                let num_scissors = scissors.len();
                assert!(0 < num_scissors && num_scissors <= self.share.limits.max_viewports);

                if num_scissors == 1 {
                    let scissor = scissors[0];
                    unsafe { gl.Scissor(scissor[0], scissor[1], scissor[2], scissor[3]) };
                } else {
                    // Support for this function is coupled with the support
                    // of multiple viewports.
                    unsafe { gl.ScissorArrayv(0, num_scissors as i32, scissors.as_ptr() as *const _) };
                }
            }
            com::Command::SetBlendColor(color) => {
                state::set_blend_color(&self.share.context, color);
            }
            com::Command::ClearBufferColorF(draw_buffer, cv) => unsafe {
                self.share.context.ClearBufferfv(gl::COLOR, draw_buffer, cv.as_ptr());
            }
            com::Command::ClearBufferColorU(draw_buffer, cv) => unsafe {
                self.share.context.ClearBufferuiv(gl::COLOR, draw_buffer, cv.as_ptr());
            }
            com::Command::ClearBufferColorI(draw_buffer, cv) => unsafe {
                self.share.context.ClearBufferiv(gl::COLOR, draw_buffer, cv.as_ptr());
            }
            com::Command::ClearBufferDepthStencil(depth, stencil) => unsafe {
                let (target, depth, stencil) = match (depth, stencil) {
                    (Some(depth), Some(stencil)) => (gl::DEPTH_STENCIL, depth, stencil),
                    (Some(depth), None) => (gl::DEPTH, depth, 0),
                    (None, Some(stencil)) => (gl::STENCIL, 0.0, stencil),
                    _ => unreachable!(),
                };

                self.share.context.ClearBufferfi(target, 0, depth, stencil as _);
            }
            com::Command::DrawBuffers(draw_buffers) => unsafe {
                let draw_buffers = Self::get::<gl::types::GLenum>(data_buf, draw_buffers);
                self.share.context.DrawBuffers(
                    draw_buffers.len() as _,
                    draw_buffers.as_ptr(),
                );
            }
            com::Command::BindFrameBuffer(point, frame_buffer) => {
                if self.share.private_caps.framebuffer {
                    let gl = &self.share.context;
                    unsafe { gl.BindFramebuffer(point, frame_buffer) };
                } else if frame_buffer != 0 {
                    error!("Tried to bind FBO {} without FBO support!", frame_buffer);
                }
            }
            com::Command::BindTargetView(point, attachment, view) => {
                self.bind_target(point, attachment, &view)
            }
            com::Command::SetDrawColorBuffers(num) => {
                state::bind_draw_color_buffers(&self.share.context, num);
            }
            com::Command::SetPatchSize(num) => unsafe {
                self.share.context.PatchParameteri(gl::PATCH_VERTICES, num);
            }
            com::Command::BindProgram(program) => unsafe {
                self.share.context.UseProgram(program);
            }
            com::Command::BindBlendSlot(slot, ref blend) => {
                state::bind_blend_slot(&self.share.context, slot, blend);
            }
            com::Command::BindAttribute(ref attribute, handle, stride, function_type) => unsafe {
                use native::VertexAttribFunction::*;

                let &native::AttributeDesc { location, size, format, offset, .. } = attribute;
                let offset = offset as *const gl::types::GLvoid;
                let gl = &self.share.context;

                gl.BindBuffer(gl::ARRAY_BUFFER, handle);

                match function_type {
                    Float => gl.VertexAttribPointer(location, size, format, gl::FALSE, stride, offset),
                    Integer => gl.VertexAttribIPointer(location, size, format, stride, offset),
                    Double => gl.VertexAttribLPointer(location, size, format, stride, offset),
                }

                gl.EnableVertexAttribArray(location);
                gl.BindBuffer(gl::ARRAY_BUFFER, 0);
            }
            /*
            com::Command::UnbindAttribute(ref attribute) => unsafe {
                self.share.context.DisableVertexAttribArray(attribute.location);
            }*/
            com::Command::CopyBufferToBuffer(src, dst, ref r) => unsafe {
                let gl = &self.share.context;
                gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, src);
                gl.BindBuffer(gl::PIXEL_PACK_BUFFER, dst);
                gl.CopyBufferSubData(
                    gl::PIXEL_UNPACK_BUFFER, gl::PIXEL_PACK_BUFFER,
                    r.src as _, r.dst as _, r.size as _,
                );
                gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
                gl.BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
            }
            com::Command::CopyBufferToTexture(buffer, texture, ref r) => unsafe {
                // TODO: Fix format and active texture
                assert_eq!(r.image_offset.z, 0);
                let gl = &self.share.context;
                gl.ActiveTexture(gl::TEXTURE0);
                gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, buffer);
                gl.BindTexture(gl::TEXTURE_2D, texture);
                gl.TexSubImage2D(
                    gl::TEXTURE_2D, r.image_layers.level as _,
                    r.image_offset.x, r.image_offset.y,
                    r.image_extent.width as _, r.image_extent.height as _,
                    gl::RGBA, gl::UNSIGNED_BYTE, ptr::null(),
                );
                gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
            }
            com::Command::CopyBufferToSurface(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyTextureToBuffer(texture, buffer, ref r) => unsafe {
                // TODO: Fix format and active texture
                // TODO: handle partial copies gracefully
                assert_eq!(r.image_offset, hal::image::Offset { x: 0, y: 0, z: 0 });
                let gl = &self.share.context;
                gl.ActiveTexture(gl::TEXTURE0);
                gl.BindBuffer(gl::PIXEL_PACK_BUFFER, buffer);
                gl.BindTexture(gl::TEXTURE_2D, texture);
                gl.GetTexImage(
                    gl::TEXTURE_2D, r.image_layers.level as _,
                    //r.image_offset.x, r.image_offset.y,
                    //r.image_extent.width as _, r.image_extent.height as _,
                    gl::RGBA, gl::UNSIGNED_BYTE, ptr::null_mut(),
                );
                gl.BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
            }
            com::Command::CopySurfaceToBuffer(..) => {
                unimplemented!() //TODO: use FBO
            }
            /*
            com::Command::BindConstantBuffer(pso::ConstantBufferParam(buffer, _, slot)) => unsafe {
                self.share.context.BindBufferBase(gl::UNIFORM_BUFFER, slot as gl::types::GLuint, buffer);
            },
            com::Command::BindResourceView(pso::ResourceViewParam(view, _, slot)) => unsafe {
                self.share.context.ActiveTexture(gl::TEXTURE0 + slot as gl::types::GLenum);
                self.share.context.BindTexture(view.bind, view.object);
            },
            com::Command::BindUnorderedView(_uav) => unimplemented!(),
            com::Command::BindSampler(pso::SamplerParam(sampler, _, slot), bind_opt) => {
                let gl = &self.share.context;
                if self.share.private_caps.sampler_objects {
                    unsafe { gl.BindSampler(slot as gl::types::GLuint, sampler.object) };
                } else {
                    assert!(hal::MAX_SAMPLERS <= hal::MAX_RESOURCE_VIEWS);
                    debug_assert_eq!(sampler.object, 0);
                    if let Some(bind) = bind_opt {
                        tex::bind_sampler(gl, bind, &sampler.info, &self.share.private_caps);
                    }else {
                        error!("Trying to bind a sampler to slot {}, when sampler objects are not supported, and no texture is bound there", slot);
                    }
                }
            },
            com::Command::BindPixelTargets(pts) => {
                let point = gl::DRAW_FRAMEBUFFER;
                for i in 0 .. hal::MAX_COLOR_TARGETS {
                    let att = gl::COLOR_ATTACHMENT0 + i as gl::types::GLuint;
                    if let Some(ref target) = pts.colors[i] {
                        self.bind_target(point, att, target);
                    } else {
                        self.unbind_target(point, att);
                    }
                }
                if let Some(ref depth) = pts.depth {
                    self.bind_target(point, gl::DEPTH_ATTACHMENT, depth);
                }
                if let Some(ref stencil) = pts.stencil {
                    self.bind_target(point, gl::STENCIL_ATTACHMENT, stencil);
                }
            },
            com::Command::BindAttribute(slot, buffer,  bel) => {
                self.bind_attribute(slot, buffer, bel);
            },
            com::Command::UnbindAttribute(slot) => unsafe {
                self.share.context.DisableVertexAttribArray(slot as gl::types::GLuint);
            },
            com::Command::BindUniform(loc, uniform) => {
                let gl = &self.share.context;
                shade::bind_uniform(gl, loc as gl::types::GLint, uniform);
            },
            com::Command::SetRasterizer(rast) => {
                state::bind_rasterizer(&self.share.context, &rast, self.share.info.version.is_embedded);
            },
            com::Command::SetDepthState(depth) => {
                state::bind_depth(&self.share.context, &depth);
            },
            com::Command::SetStencilState(stencil, refs, cull) => {
                state::bind_stencil(&self.share.context, &stencil, refs, cull);
            },
            com::Command::SetBlendState(slot, color) => {
                if self.share.capabilities.separate_blending_slots {
                    state::bind_blend_slot(&self.share.context, slot, color);
                }else if slot == 0 {
                    //self.temp.color = color; //TODO
                    state::bind_blend(&self.share.context, color);
                }else if false {
                    error!("Separate blending slots are not supported");
                }
            },
            com::Command::CopyBuffer(src, dst, src_offset, dst_offset, size) => {
                let gl = &self.share.context;

                if self.share.capabilities.copy_buffer {
                    unsafe {
                        gl.BindBuffer(gl::COPY_READ_BUFFER, src);
                        gl.BindBuffer(gl::COPY_WRITE_BUFFER, dst);
                        gl.CopyBufferSubData(gl::COPY_READ_BUFFER,
                                            gl::COPY_WRITE_BUFFER,
                                            src_offset,
                                            dst_offset,
                                            size);
                    }
                } else {
                    debug_assert!(self.share.private_caps.buffer_storage == false);

                    unsafe {
                        let mut src_ptr = 0 as *mut ::std::os::raw::c_void;
                        device::temporary_ensure_mapped(&mut src_ptr, gl::COPY_READ_BUFFER, src, memory::READ, gl);
                        src_ptr.offset(src_offset);

                        let mut dst_ptr = 0 as *mut ::std::os::raw::c_void;
                        device::temporary_ensure_mapped(&mut dst_ptr, gl::COPY_WRITE_BUFFER, dst, memory::WRITE, gl);
                        dst_ptr.offset(dst_offset);

                        ::std::ptr::copy(src_ptr, dst_ptr, size as usize);

                        device::temporary_ensure_unmapped(&mut src_ptr, gl::COPY_READ_BUFFER, src, gl);
                        device::temporary_ensure_unmapped(&mut dst_ptr, gl::COPY_WRITE_BUFFER, dst, gl);
                    }
                }
            },
            */
        }
        if let Err(err) = self.share.check() {
            panic!("Error {:?} executing command: {:?}", err, cmd)
        }
    }
    fn signal_fence(&mut self, fence: &native::Fence) {
        if self.share.private_caps.sync {
            let gl = &self.share.context;
            let sync = unsafe {
                gl.FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0)
            };

            fence.0.set(sync);
        }
    }
}

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(
        &mut self,
        submit_info: hal::queue::RawSubmission<Backend, IC>,
        fence: Option<&native::Fence>,
    ) where
        IC: IntoIterator,
        IC::Item: Borrow<com::RawCommandBuffer>,
    {
        use pool::BufferMemory;
        {
            for buf in submit_info.cmd_buffers {
                let cb = buf.borrow();
                let memory = cb
                    .memory
                    .try_lock()
                    .expect("Trying to submit a command buffers, while memory is in-use.");

                let buffer = match *memory {
                    BufferMemory::Linear(ref buffer) => buffer,
                    BufferMemory::Individual { ref storage, .. } => {
                        storage.get(&cb.id).unwrap()
                    }
                };

                assert!(buffer.commands.len() >= (cb.buf.offset+cb.buf.size) as usize);
                let commands = &buffer.commands[cb.buf.offset as usize..(cb.buf.offset+cb.buf.size) as usize];
                self.reset_state();
                for com in commands {
                    self.process(com, &buffer.data);
                }
            }
        }
        fence.map(|fence| self.signal_fence(fence));
    }

    #[cfg(feature = "glutin")]
    fn present<IS, IW>(&mut self, swapchains: IS, _wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<window::glutin::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<native::Semaphore>,
    {
        use glutin::GlContext;

        for swapchain in swapchains {
            swapchain
                .borrow()
                .window
                .swap_buffers()
                .unwrap();
        }
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unsafe { self.share.context.Finish(); }
        Ok(())
    }
}
