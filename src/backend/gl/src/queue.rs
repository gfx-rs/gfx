use crate::Starc;
use std::borrow::Borrow;
use std::{mem, slice};
use crate::Starc;

use crate::hal;
use crate::hal::error;

use glow::Context;
use smallvec::SmallVec;

use crate::info::LegacyFeatures;
use crate::{command as com, device, native, state, window};
use crate::{Backend, GlContext, Share};

// State caching system for command queue.
//
// We track the current global state, which is based on
// the restriction that we only expose _one_ command queue.
//
// This allows us to minimize additional driver calls to
// ensure that command buffers are handled isolated of each other.
#[derive(Debug)]
struct State {
    // Indicate if the vertex array object is bound.
    // If VAOs are not supported, this will be also set to true.
    vao: bool,
    // Currently bound index/element buffer.
    // None denotes that we don't know what is currently bound.
    index_buffer: Option<native::RawBuffer>,
    // Currently set viewports.
    num_viewports: usize,
    // Currently set scissor rects.
    num_scissors: usize,
    // Currently bound fbo
    fbo: gl::types::GLuint,
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
            fbo: 0,
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

#[derive(Debug)]
pub struct CommandQueue {
    pub(crate) share: Starc<Share>,
    vao: Option<native::VertexArray>,
    state: State,
}

impl CommandQueue {
    /// Create a new command queue.
    pub(crate) fn new(share: &Starc<Share>, vao: Option<native::VertexArray>) -> Self {
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
    pub unsafe fn with_gl<F: FnMut(&GlContext)>(&mut self, mut fun: F) {
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
                (glow::BYTE, glow::SHORT, glow::INT),
            C::Uint | C::Unorm =>
                (glow::UNSIGNED_BYTE, glow::UNSIGNED_SHORT, glow::UNSIGNED_INT),
            C::Float => (glow::ZERO, glow::HALF_FLOAT, glow::FLOAT),
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
        unsafe { gl.BindBuffer(glow::ARRAY_BUFFER, buffer) };
        let offset = bel.elem.offset as *const glow::types::GLvoid;
        let stride = bel.desc.stride as i32;
        match bel.elem.format.1 {
            C::Int | C::Uint => unsafe {
                gl.VertexAttribIPointer(slot as glow::types::GLuint,
                    count, gl_type, stride, offset);
            },
            C::Inorm | C::Unorm => unsafe {
                gl.VertexAttribPointer(slot as glow::types::GLuint,
                    count, gl_type, glow::TRUE, stride, offset);
            },
            //C::Sscaled | C::Uscaled => unsafe {
            //    gl.VertexAttribPointer(slot as glow::types::GLuint,
            //        count, gl_type, glow::FALSE, stride, offset);
            //},
            C::Float => unsafe {
                gl.VertexAttribPointer(slot as glow::types::GLuint,
                    count, gl_type, glow::FALSE, stride, offset);
            },
            C::Srgb => (),
        }
        unsafe { gl.EnableVertexAttribArray(slot as glow::types::GLuint) };
        if self.share.capabilities.instance_rate {
            unsafe { gl.VertexAttribDivisor(slot as glow::types::GLuint,
                bel.desc.rate as glow::types::GLuint) };
        } else if bel.desc.rate != 0 {
            error!("Instanced arrays are not supported");
        }
    }
    */

    fn bind_target(&mut self, point: u32, attachment: u32, view: &native::ImageView) {
        let gl = &self.share.context;
        match view {
            &native::ImageView::Surface(surface) => unsafe {
                gl.framebuffer_renderbuffer(point, attachment, glow::RENDERBUFFER, Some(surface));
            },
            &native::ImageView::Texture(texture, level) => unsafe {
                gl.framebuffer_texture(point, attachment, Some(texture), level as i32);
            },
            &native::ImageView::TextureLayer(texture, level, layer) => unsafe {
                gl.framebuffer_texture_layer(
                    point,
                    attachment,
                    Some(texture),
                    level as i32,
                    layer as i32,
                );
            },
        }
    }

    fn _unbind_target(&mut self, point: u32, attachment: u32) {
        let gl = &self.share.context;
        // TODO: Find workaround or use explicit `textarget` with the other `framebuffer_texture`
        unsafe { gl.framebuffer_texture(point, attachment, None, 0) };
    }

    /// Return a reference to a stored data object.
    fn get<T>(data: &[u8], ptr: com::BufferSlice) -> &[T] {
        let u32_size = mem::size_of::<T>();
        assert_eq!(ptr.size % u32_size as u32, 0);
        let raw = Self::get_raw(data, ptr);
        unsafe { slice::from_raw_parts(raw.as_ptr() as *const _, raw.len() / u32_size) }
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

        // Bind default VAO
        if !self.state.vao {
            if self.share.private_caps.vertex_array {
                unsafe { gl.bind_vertex_array(self.vao) };
            }
            self.state.vao = true
        }

        // Reset indirect draw buffer
        if self
            .share
            .legacy_features
            .contains(LegacyFeatures::INDIRECT_EXECUTION)
        {
            unsafe { gl.bind_buffer(glow::DRAW_INDIRECT_BUFFER, None) };
        }

        // Unbind index buffers
        // TODO: Handle already unbound case
        unsafe { gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None) };
        self.state.index_buffer = None;

        // Reset viewports
        if self.state.num_viewports == 1 {
            unsafe {
                gl.viewport(0, 0, 0, 0);
                gl.depth_range_f32(0.0, 1.0);
            };
        } else if self.state.num_viewports > 1 {
            // 16 viewports is a common limit set in drivers.
            let viewports: SmallVec<[[f32; 4]; 16]> = (0..self.state.num_viewports)
                .map(|_| [0.0, 0.0, 0.0, 0.0])
                .collect();
            let depth_ranges: SmallVec<[[f64; 2]; 16]> =
                (0..self.state.num_viewports).map(|_| [0.0, 0.0]).collect();
            unsafe {
                gl.viewport_f32_slice(0, viewports.len() as i32, &viewports);
                gl.depth_range_f64_slice(0, depth_ranges.len() as i32, &depth_ranges);
            }
        }

        // Reset scissors
        if self.state.num_scissors == 1 {
            unsafe { gl.scissor(0, 0, 0, 0) };
        } else if self.state.num_scissors > 1 {
            // 16 viewports is a common limit set in drivers.
            let scissors: SmallVec<[[i32; 4]; 16]> =
                (0..self.state.num_scissors).map(|_| [0, 0, 0, 0]).collect();
            unsafe { gl.scissor_slice(0, scissors.len() as i32, scissors.as_slice()) };
        }
    }

    fn process(&mut self, cmd: &com::Command, data_buf: &[u8]) {
        match *cmd {
            com::Command::BindIndexBuffer(buffer) => {
                let gl = &self.share.context;
                self.state.index_buffer = Some(buffer);
                unsafe { gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(buffer)) };
            }
            //          com::Command::BindVertexBuffers(_data_ptr) =>
            com::Command::Draw {
                primitive,
                ref vertices,
                ref instances,
            } => {
                let gl = &self.share.context;
                let legacy = &self.share.legacy_features;
                if instances == &(0u32..1) {
                    unsafe {
                        gl.draw_arrays(
                            primitive,
                            vertices.start as _,
                            (vertices.end - vertices.start) as _,
                        );
                    }
                } else if legacy.contains(LegacyFeatures::DRAW_INSTANCED) {
                    if instances.start == 0 {
                        unsafe {
                            gl.draw_arrays_instanced(
                                primitive,
                                vertices.start as _,
                                (vertices.end - vertices.start) as _,
                                instances.end as _,
                            );
                        }
                    } else if legacy.contains(LegacyFeatures::DRAW_INSTANCED_BASE) {
                        unsafe {
                            gl.draw_arrays_instanced_base_instance(
                                primitive,
                                vertices.start as _,
                                (vertices.end - vertices.start) as _,
                                (instances.end - instances.start) as _,
                                instances.start as _,
                            );
                        }
                    } else {
                        error!(
                            "Instanced draw calls with non-zero base instance are not supported"
                        );
                    }
                } else {
                    error!("Instanced draw calls are not supported");
                }
            }
            com::Command::DrawIndexed {
                primitive,
                index_type,
                index_count,
                index_buffer_offset,
                base_vertex,
                ref instances,
            } => {
                let gl = &self.share.context;
                let legacy = &self.share.legacy_features;

                if instances == &(0u32..1) {
                    if base_vertex == 0 {
                        unsafe {
                            gl.draw_elements(
                                primitive,
                                index_count as _,
                                index_type,
                                index_buffer_offset as i32,
                            );
                        }
                    } else if legacy.contains(LegacyFeatures::DRAW_INDEXED_BASE) {
                        unsafe {
                            gl.draw_elements_base_vertex(
                                primitive,
                                index_count as _,
                                index_type,
                                index_buffer_offset as i32,
                                base_vertex as _,
                            );
                        }
                    } else {
                        error!("Base vertex with indexed drawing not supported");
                    }
                } else if legacy.contains(LegacyFeatures::DRAW_INDEXED_INSTANCED) {
                    if base_vertex == 0 && instances.start == 0 {
                        unsafe {
                            gl.draw_elements_instanced(
                                primitive,
                                index_count as _,
                                index_type,
                                index_buffer_offset as i32,
                                instances.end as _,
                            );
                        }
                    } else if instances.start == 0
                        && legacy.contains(LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE_VERTEX)
                    {
                        unsafe {
                            gl.draw_elements_instanced_base_vertex(
                                primitive,
                                index_count as _,
                                index_type,
                                index_buffer_offset as i32,
                                instances.end as _,
                                base_vertex as _,
                            );
                        }
                    } else if instances.start == 0 {
                        error!("Base vertex with instanced indexed drawing is not supported");
                    } else if legacy.contains(LegacyFeatures::DRAW_INDEXED_INSTANCED_BASE) {
                        unsafe {
                            gl.draw_elements_instanced_base_vertex_base_instance(
                                primitive,
                                index_count as _,
                                index_type,
                                index_buffer_offset as i32,
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
                unsafe { gl.dispatch_compute(count[0], count[1], count[2]) };
            }
            com::Command::DispatchIndirect(buffer, offset) => {
                // Capability support is given by which queue types will be exposed.
                // If there is no compute support, this pattern should never be reached
                // because no queue with compute capability can be created.
                let gl = &self.share.context;
                unsafe {
                    gl.bind_buffer(glow::DRAW_INDIRECT_BUFFER, Some(buffer));
                    // TODO: possible integer conversion issue
                    gl.dispatch_compute_indirect(offset as _);
                }
            }
            com::Command::SetViewports {
                first_viewport,
                viewport_ptr,
                depth_range_ptr,
            } => {
                let gl = &self.share.context;
                let viewports = Self::get::<[f32; 4]>(data_buf, viewport_ptr);
                let depth_ranges = Self::get::<[f64; 2]>(data_buf, depth_range_ptr);

                let num_viewports = viewports.len();
                assert_eq!(num_viewports, depth_ranges.len());
                assert!(0 < num_viewports && num_viewports <= self.share.limits.max_viewports);

                if num_viewports == 1 {
                    let view = viewports[0];
                    let depth_range  = depth_ranges[0];
                    unsafe {
                        gl.viewport(
                            view[0] as i32,
                            view[1] as i32,
                            view[2] as i32,
                            view[3] as i32,
                        );
                        #[cfg(not(target_arch = "wasm32"))] // TODO
                        gl.depth_range_f64(depth_range[0], depth_range[1]);
                    };
                } else if num_viewports > 1 {
                    // Support for these functions is coupled with the support
                    // of multiple viewports.
                    unsafe {
                        gl.viewport_f32_slice(first_viewport, num_viewports as i32, &viewports);
                        gl.depth_range_f64_slice(first_viewport, num_viewports as i32, &depth_ranges);
                    };
                }
            }
            com::Command::SetScissors(first_scissor, data_ptr) => {
                let gl = &self.share.context;
                let scissors = Self::get::<[i32; 4]>(data_buf, data_ptr);
                let num_scissors = scissors.len();
                assert!(0 < num_scissors && num_scissors <= self.share.limits.max_viewports);

                if num_scissors == 1 {
                    let scissor = scissors[0];
                    unsafe { gl.scissor(scissor[0], scissor[1], scissor[2], scissor[3]) };
                } else {
                    // Support for this function is coupled with the support
                    // of multiple viewports.
                    unsafe { gl.scissor_slice(first_scissor, num_scissors as i32, scissors) };
                }
            }
            com::Command::SetBlendColor(color) => {
                state::set_blend_color(&self.share.context, color);
            }
            com::Command::ClearBufferColorF(draw_buffer, mut cv) => unsafe {
                self.share
                    .context
                    .clear_buffer_f32_slice(glow::COLOR, draw_buffer, cv.as_ptr());
            }
            com::Command::ClearBufferColorU(draw_buffer, mut cv) => unsafe {
                self.share
                    .context
                    .clear_buffer_u32_slice(glow::COLOR, draw_buffer, cv.as_ptr());
            }
            com::Command::ClearBufferColorI(draw_buffer, mut cv) => unsafe {
                self.share
                    .context
                    .clear_buffer_i32_slice(glow::COLOR, draw_buffer, cv.as_ptr());
            }
            com::Command::ClearBufferDepthStencil(depth, stencil) => unsafe {
                match (depth, stencil) {
                    (Some(depth), Some(stencil)) => {
                        self.share
                            .context
                            .clear_buffer_depth_stencil(glow::DEPTH_STENCIL, 0, depth, stencil as _);
                    },
                    (Some(depth), None) => {
                        let mut depths = [depth];
                        self.share
                            .context
                            .clear_buffer_f32_slice(glow::DEPTH, 0, &mut depths);
                    },
                    (None, Some(stencil)) => {
                        let mut stencils = [stencil];
                        self.share
                            .context
                            .clear_buffer_i32_slice(glow::STENCIL, 0, &mut stencils));
                    }
                    _ => unreachable!(),
                };
            },
            com::Command::ClearTexture(_color) => unimplemented!(),
            com::Command::DrawBuffers(draw_buffers) => unsafe {
                #[cfg(not(target_arch = "wasm32"))] // TODO
                {
                    let draw_buffers = Self::get::<u32>(data_buf, draw_buffers);
                    self.share.context.draw_buffers(draw_buffers);
                }
            }
            com::Command::BindFrameBuffer(point, frame_buffer) => {
                if self.share.private_caps.framebuffer {
                    let gl = &self.share.context;
                    unsafe { gl.bind_framebuffer(point, frame_buffer) };
                    self.state.fbo = frame_buffer;
                } else if frame_buffer.is_some() {
                    error!("Tried to bind FBO without FBO support!");
                }
            }
            com::Command::BindTargetView(point, attachment, view) => {
                self.bind_target(point, attachment, &view)
            }
            com::Command::SetDrawColorBuffers(num) => {
                state::bind_draw_color_buffers(&self.share.context, num);
            }
            com::Command::SetPatchSize(num) => unsafe {
                self.share.context.patch_parameter_i32(glow::PATCH_VERTICES, num);
            }
            com::Command::BindProgram(program) => unsafe {
                self.share.context.use_program(Some(program));
            }
            com::Command::BindBlendSlot(slot, ref blend) => {
                state::bind_blend_slot(&self.share.context, slot, blend);
            }
            com::Command::BindAttribute(ref attribute, handle, stride, rate) => unsafe {
                use crate::native::VertexAttribFunction::*;

                let &native::AttributeDesc {
                    location,
                    size,
                    format,
                    offset,
                    vertex_attrib_fn,
                    ..
                } = attribute;
                let gl = &self.share.context;

                gl.bind_buffer(glow::ARRAY_BUFFER, Some(handle));

                match vertex_attrib_fn {
                    Float => {
                        gl.vertex_attrib_pointer_f32(location, size, format, false, stride, offset as i32)
                    }
                    Integer => gl.vertex_attrib_pointer_i32(location, size, format, stride, offset as i32),
                    Double => gl.vertex_attrib_pointer_f64(location, size, format, stride, offset as i32),
                }

                if rate != 0 {
                    if self.share.legacy_features.contains(LegacyFeatures::INSTANCED_ATTRIBUTE_BINDING) {
                        gl.vertex_attrib_divisor(location, rate);
                    } else {
                        error!("Binding attribute with instanced input rate is not supported");
                    }
                }

                gl.enable_vertex_attrib_array(location);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
            /*
            com::Command::UnbindAttribute(ref attribute) => unsafe {
                self.share.context.DisableVertexAttribArray(attribute.location);
            }*/
            com::Command::CopyBufferToBuffer(src, dst, ref r) => unsafe {
                let gl = &self.share.context;
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(src));
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(dst));
                gl.copy_buffer_sub_data(
                    glow::PIXEL_UNPACK_BUFFER,
                    glow::PIXEL_PACK_BUFFER,
                    r.src as _,
                    r.dst as _,
                    r.size as _,
                );
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            }
            com::Command::CopyBufferToTexture(buffer, texture, ref r) => unsafe {
                // TODO: Fix format and active texture
                assert_eq!(r.image_offset.z, 0);
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0);
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(buffer));
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_sub_image_2d_pixel_buffer_offset(
                    glow::TEXTURE_2D,
                    r.image_layers.level as _,
                    r.image_offset.x,
                    r.image_offset.y,
                    r.image_extent.width as _,
                    r.image_extent.height as _,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    0,
                );
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            }
            com::Command::CopyBufferToSurface(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyTextureToBuffer(texture, buffer, ref r) => unsafe {
                // TODO: Fix format and active texture
                // TODO: handle partial copies gracefully
                assert_eq!(r.image_offset, hal::image::Offset { x: 0, y: 0, z: 0 });
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0);
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(buffer));
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.get_tex_image(
                    glow::TEXTURE_2D, r.image_layers.level as _,
                    //r.image_offset.x,
                    //r.image_offset.y,
                    //r.image_extent.width as _,
                    //r.image_extent.height as _,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    None,
                );
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            }
            com::Command::CopySurfaceToBuffer(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyImageToTexture(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyImageToSurface(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::BindBufferRange(target, index, buffer, offset, size) => unsafe {
                let gl = &self.share.context;
                gl.bind_buffer_range(target, index, Some(buffer), offset, size);
            }
            com::Command::BindTexture(index, texture) => unsafe {
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0 + index);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            }
            com::Command::BindSampler(index, sampler) => unsafe {
                let gl = &self.share.context;
                gl.bind_sampler(index, Some(sampler));
            }
            com::Command::SetTextureSamplerSettings(index, texture, ref sinfo) => unsafe {
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0 + index);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));

                // TODO: Optimization: only change texture properties that have changed.
                device::set_sampler_info(
                    &self.share,
                    &sinfo,
                    |a, b| gl.tex_parameter_f32(glow::TEXTURE_2D, a, b),
                    |a, b| gl.tex_parameter_f32_slice(glow::TEXTURE_2D, a, &b),
                    |a, b| gl.tex_parameter_i32(glow::TEXTURE_2D, a, b),
                );
            }, /*
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
            },*/
            com::Command::BindUniform { uniform, buffer } => {
                let gl = &self.share.context;

                unsafe {
                    match uniform.utype {
                        gl::FLOAT => {
                            let data = Self::get::<f32>(data_buf, buffer);
                            gl.Uniform1fv(uniform.location as _, 1, data.as_ptr() as _);
                        }
                        gl::FLOAT_VEC2 => {
                            let data = Self::get::<[f32; 2]>(data_buf, buffer);
                            gl.Uniform2fv(uniform.location as _, 1, data[0].as_ptr() as _);
                        }
                        gl::FLOAT_VEC3 => {
                            let data = Self::get::<[f32; 3]>(data_buf, buffer);
                            gl.Uniform3fv(uniform.location as _, 1, data[0].as_ptr() as _);
                        }
                        gl::FLOAT_VEC4 => {
                            let data = Self::get::<[f32; 4]>(data_buf, buffer);
                            gl.Uniform4fv(uniform.location as _, 1, data[0].as_ptr() as _);
                        }
                        gl::INT => {
                            let data = Self::get::<i32>(data_buf, buffer);
                            gl.Uniform1iv(uniform.location as _, 1, data.as_ptr() as _);
                        }
                        gl::INT_VEC2 => {
                            let data = Self::get::<[i32; 2]>(data_buf, buffer);
                            gl.Uniform2iv(uniform.location as _, 1, data[0].as_ptr() as _);
                        }
                        gl::INT_VEC3 => {
                            let data = Self::get::<[i32; 3]>(data_buf, buffer);
                            gl.Uniform3iv(uniform.location as _, 1, data[0].as_ptr() as _);
                        }
                        gl::INT_VEC4 => {
                            let data = Self::get::<[i32; 4]>(data_buf, buffer);
                            gl.Uniform4iv(uniform.location as _, 1, data[0].as_ptr() as _);
                        }
                        gl::FLOAT_MAT2 => {
                            let data = Self::get::<[f32; 4]>(data_buf, buffer);
                            gl.UniformMatrix2fv(
                                uniform.location as _,
                                1,
                                gl::FALSE,
                                data[0].as_ptr(),
                            );
                        }
                        gl::FLOAT_MAT3 => {
                            let data = Self::get::<[f32; 9]>(data_buf, buffer);
                            gl.UniformMatrix3fv(
                                uniform.location as _,
                                1,
                                gl::FALSE,
                                data[0].as_ptr(),
                            );
                        }
                        gl::FLOAT_MAT4 => {
                            let data = Self::get::<[f32; 16]>(data_buf, buffer);
                            gl.UniformMatrix4fv(
                                uniform.location as _,
                                1,
                                gl::FALSE,
                                data[0].as_ptr(),
                            );
                        }
                        _ => panic!("Unsupported uniform datatype!"),
                    }
                }
            } 
            com::Command::BindRasterizer { rasterizer } => { 
                use crate::hal::pso::FrontFace::*;
                use crate::hal::pso::PolygonMode::*;
                
                let gl = &self.share.context;
                
                unsafe {
                    gl.FrontFace(match rasterizer.front_face {
                        Clockwise => gl::CW,
                        CounterClockwise => gl::CCW,
                    })
                };

                if !rasterizer.cull_face.is_empty() {
                    unsafe {
                        gl.Enable(gl::CULL_FACE);
                        gl.CullFace(match rasterizer.cull_face {
                            hal::pso::Face::FRONT => gl::FRONT,
                            hal::pso::Face::BACK => gl::BACK,
                            _ => gl::FRONT_AND_BACK,
                        });
                    }
                } else {
                    unsafe {
                        gl.Disable(gl::CULL_FACE);
                    }
                }

                let (gl_draw, gl_offset) = match rasterizer.polygon_mode {
                    Point => (gl::POINT, gl::POLYGON_OFFSET_POINT),
                    Line(width) => {
                        unsafe { gl.LineWidth(width) };
                        (gl::LINE, gl::POLYGON_OFFSET_LINE)
                    }
                    Fill => (gl::FILL, gl::POLYGON_OFFSET_FILL),
                };

                unsafe { gl.PolygonMode(gl::FRONT_AND_BACK, gl_draw) };

                match rasterizer.depth_bias {
                    Some(hal::pso::State::Static(bias)) => unsafe {
                        gl.Enable(gl_offset);
                        gl.PolygonOffset(bias.slope_factor as _, bias.const_factor as _);
                    },
                    _ => unsafe { gl.Disable(gl_offset) },
                }

                match false {
                    //TODO
                    true => unsafe { gl.Enable(gl::MULTISAMPLE) },
                    false => unsafe { gl.Disable(gl::MULTISAMPLE) },
                }
            }
            com::Command::BindDepth { depth } => {
                use crate::hal::pso::Comparison::*;
                
                let gl = &self.share.context;
                
                match depth {
                    hal::pso::DepthTest::On { fun, write } => unsafe {
                        gl.Enable(gl::DEPTH_TEST);

                        let cmp = match fun {
                            Never => gl::NEVER,
                            Less => gl::LESS,
                            LessEqual => gl::LEQUAL,
                            Equal => gl::EQUAL,
                            GreaterEqual => gl::GEQUAL,
                            Greater => gl::GREATER,
                            NotEqual => gl::NOTEQUAL,
                            Always => gl::ALWAYS,
                        };

                        gl.DepthFunc(cmp);
                        gl.DepthMask(write as _);
                    },
                    hal::pso::DepthTest::Off => unsafe {
                        gl.Disable(gl::DEPTH_TEST);
                    },
                }
            }
            /*
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
            let sync = if self.share.private_caps.sync {
                Some(unsafe { gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0).unwrap() })
            } else {
                None
            };

            fence.0.set(sync);
        }
    }
}

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        submit_info: hal::queue::Submission<Ic, Iw, Is>,
        fence: Option<&native::Fence>,
    ) where
        T: 'a + Borrow<com::RawCommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, hal::pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        use crate::pool::BufferMemory;
        {
            for buf in submit_info.command_buffers {
                let cb = buf.borrow();
                let memory = cb
                    .memory
                    .try_lock()
                    .expect("Trying to submit a command buffers, while memory is in-use.");

                let buffer = match *memory {
                    BufferMemory::Linear(ref buffer) => buffer,
                    BufferMemory::Individual { ref storage, .. } => storage.get(&cb.id).unwrap(),
                };

                assert!(buffer.commands.len() >= (cb.buf.offset + cb.buf.size) as usize);
                let commands = &buffer.commands
                    [cb.buf.offset as usize..(cb.buf.offset + cb.buf.size) as usize];
                self.reset_state();
                for com in commands {
                    self.process(com, &buffer.data);
                }
            }
        }
        fence.map(|fence| self.signal_fence(fence));
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
    unsafe fn present<'a, W, Is, S, Iw>(
        &mut self,
        swapchains: Is,
        _wait_semaphores: Iw,
    ) -> Result<Option<hal::window::Suboptimal>, hal::window::PresentError>
    where
        W: 'a + Borrow<window::glutin::Swapchain>,
        Is: IntoIterator<Item = (&'a W, hal::SwapImageIndex)>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = &'a S>,
    {
        let gl = &self.share.context;

        for swapchain in swapchains {
            let extent = swapchain.0.borrow().extent;

            gl.BindFramebuffer(gl::READ_FRAMEBUFFER, self.state.fbo);
            gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
            gl.BlitFramebuffer(

                0,
                0,
                extent.width as _,
                extent.height as _,
                0,
                0,
                extent.width as _,
                extent.height as _,
                gl::COLOR_BUFFER_BIT,
                gl::LINEAR,
            );

            swapchain.0.borrow().window.swap_buffers().unwrap();
        }

        Ok(None)
    }

    #[cfg(target_arch = "wasm32")]
    unsafe fn present<'a, W, Is, S, Iw>(&mut self, swapchains: Is, _wait_semaphores: Iw) -> Result<(), ()>
    where
        W: 'a + Borrow<window::web::Swapchain>,
        Is: IntoIterator<Item = (&'a W, hal::SwapImageIndex)>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = &'a S>,
    {
        // Presenting and swapping window buffers is automatic
        Ok(())
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unsafe {
            self.share.context.finish();
        }
        Ok(())
    }
}
