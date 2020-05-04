use std::borrow::Borrow;
use std::{mem, slice};

use glow::HasContext;
use smallvec::SmallVec;

use crate::{
    command as com,
    device,
    info::LegacyFeatures,
    native,
    state,
    Backend,
    GlContext,
    Share,
    Starc,
    Surface,
    Swapchain,
};

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
    fbo: Option<native::RawFrameBuffer>,
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
            fbo: None,
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
    features: hal::Features,
    vao: Option<native::VertexArray>,
    state: State,
}

impl CommandQueue {
    /// Create a new command queue.
    pub(crate) fn new(
        share: &Starc<Share>,
        features: hal::Features,
        vao: Option<native::VertexArray>,
    ) -> Self {
        CommandQueue {
            share: share.clone(),
            features,
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
            &native::ImageView::Renderbuffer(renderbuffer) => unsafe {
                gl.framebuffer_renderbuffer(
                    point,
                    attachment,
                    glow::RENDERBUFFER,
                    Some(renderbuffer),
                );
            },
            &native::ImageView::Texture(texture, _, level) => unsafe {
                gl.framebuffer_texture(point, attachment, Some(texture), level as i32);
            },
            &native::ImageView::TextureLayer(texture, _, level, layer) => unsafe {
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

    fn present_by_copy(&self, swapchain: &Swapchain, index: hal::window::SwapImageIndex) {
        let gl = &self.share.context;
        let extent = swapchain.extent;

        #[cfg(wgl)]
        swapchain.make_current();

        #[cfg(surfman)]
        gl.surfman_device
            .write()
            .make_context_current(&swapchain.context.read())
            .unwrap();

        // Use the framebuffer from the surfman context
        #[cfg(surfman)]
        let fbo = gl
            .surfman_device
            .read()
            .context_surface_info(&swapchain.context.read())
            .unwrap()
            .unwrap()
            .framebuffer_object;

        unsafe {
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(swapchain.fbos[index as usize]));
            gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                #[cfg(surfman)]
                match fbo {
                    0 => None,
                    other => Some(other),
                },
                #[cfg(not(surfman))]
                None,
            );
            gl.blit_framebuffer(
                0,
                0,
                extent.width as _,
                extent.height as _,
                0,
                0,
                extent.width as _,
                extent.height as _,
                glow::COLOR_BUFFER_BIT,
                glow::LINEAR,
            );
        }

        // Present the surfman surface
        #[cfg(surfman)]
        {
            let mut surface = gl
                .surfman_device
                .read()
                .unbind_surface_from_context(&mut swapchain.context.write())
                .expect("TODO")
                .expect("TODO");
            gl.surfman_device
                .read()
                .present_surface(&gl.surfman_context.read(), &mut surface)
                .expect("TODO");
            gl.surfman_device
                .read()
                .bind_surface_to_context(&mut swapchain.context.write(), surface)
                .expect("TODO")
        }

        #[cfg(glutin)]
        swapchain.context.swap_buffers().unwrap();

        #[cfg(wgl)]
        swapchain.swap_buffers();
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
            let depth_ranges: SmallVec<[[f64; 2]; 16]> = (0..self.state.num_viewports)
                .map(|_| [0.0, 0.0])
                .collect();
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
            let scissors: SmallVec<[[i32; 4]; 16]> = (0..self.state.num_scissors)
                .map(|_| [0, 0, 0, 0])
                .collect();
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
                let hints = &self.share.hints;

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
                    } else if hints.contains(hal::Hints::BASE_VERTEX_INSTANCE_DRAWING) {
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
                    let depth_range = depth_ranges[0];
                    unsafe {
                        gl.viewport(
                            view[0] as i32,
                            view[1] as i32,
                            view[2] as i32,
                            view[3] as i32,
                        );
                        if self.share.private_caps.depth_range_f64_precision {
                            gl.depth_range_f64(depth_range[0], depth_range[1]);
                        } else {
                            debug!("Depth ranges with f64 precision are not supported, falling back to f32");
                            gl.depth_range_f32(depth_range[0] as f32, depth_range[1] as f32);
                        }
                    };
                } else if num_viewports > 1 {
                    // Support for these functions is coupled with the support
                    // of multiple viewports.
                    unsafe {
                        gl.viewport_f32_slice(first_viewport, num_viewports as i32, &viewports);
                        gl.depth_range_f64_slice(
                            first_viewport,
                            num_viewports as i32,
                            &depth_ranges,
                        );
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
                    .clear_buffer_f32_slice(glow::COLOR, draw_buffer, &mut cv);
            },
            com::Command::ClearBufferColorU(draw_buffer, mut cv) => unsafe {
                self.share
                    .context
                    .clear_buffer_u32_slice(glow::COLOR, draw_buffer, &mut cv);
            },
            com::Command::ClearBufferColorI(draw_buffer, mut cv) => unsafe {
                self.share
                    .context
                    .clear_buffer_i32_slice(glow::COLOR, draw_buffer, &mut cv);
            },
            com::Command::ClearBufferDepthStencil(depth, stencil) => unsafe {
                let gl = &self.share.context;
                match (depth, stencil) {
                    (Some(depth), Some(stencil)) => {
                        gl.clear_buffer_depth_stencil(glow::DEPTH_STENCIL, 0, depth, stencil as _);
                    }
                    (Some(depth), None) => {
                        let mut depths = [depth];
                        gl.clear_buffer_f32_slice(glow::DEPTH, 0, &mut depths);
                    }
                    (None, Some(stencil)) => {
                        let mut stencils = [stencil as i32];
                        gl.clear_buffer_i32_slice(glow::STENCIL, 0, &mut stencils[..]);
                    }
                    _ => unreachable!(),
                };
            },
            com::Command::ClearTexture(_color) => unimplemented!(),
            com::Command::DrawBuffers(draw_buffers) => unsafe {
                if self.share.private_caps.draw_buffers {
                    let draw_buffers = Self::get::<u32>(data_buf, draw_buffers);
                    self.share.context.draw_buffers(draw_buffers);
                } else {
                    warn!("Draw buffers are not supported");
                }
            },
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
                self.share
                    .context
                    .patch_parameter_i32(glow::PATCH_VERTICES, num);
            },
            com::Command::BindProgram(program) => unsafe {
                self.share.context.use_program(Some(program));
            },
            com::Command::SetBlend(ref blend) => {
                state::set_blend(&self.share.context, blend);
            }
            com::Command::SetBlendSlot(slot, ref blend) => {
                if self.share.private_caps.draw_buffers {
                    state::set_blend_slot(&self.share.context, slot, blend, &self.features);
                } else {
                    warn!("Draw buffers are not supported");
                }
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
                    Float => gl.vertex_attrib_pointer_f32(
                        location,
                        size,
                        format,
                        false,
                        stride,
                        offset as i32,
                    ),
                    Integer => {
                        gl.vertex_attrib_pointer_i32(location, size, format, stride, offset as i32)
                    }
                    Double => {
                        gl.vertex_attrib_pointer_f64(location, size, format, stride, offset as i32)
                    }
                }

                if self
                    .share
                    .legacy_features
                    .contains(LegacyFeatures::INSTANCED_ATTRIBUTE_BINDING)
                {
                    gl.vertex_attrib_divisor(location, rate);
                } else if rate > 0 {
                    error!("Binding attribute with instanced input rate is not supported");
                }

                gl.enable_vertex_attrib_array(location);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            },
            /*
            com::Command::UnbindAttribute(ref attribute) => unsafe {
                self.share.context.DisableVertexAttribArray(attribute.location);
            }*/
            com::Command::CopyBufferToBuffer(src, dst, ref r) => unsafe {
                let gl = &self.share.context;
                gl.bind_buffer(glow::COPY_READ_BUFFER, Some(src));
                gl.bind_buffer(glow::COPY_WRITE_BUFFER, Some(dst));
                gl.copy_buffer_sub_data(
                    glow::COPY_READ_BUFFER,
                    glow::COPY_WRITE_BUFFER,
                    r.src as _,
                    r.dst as _,
                    r.size as _,
                );
                gl.bind_buffer(glow::COPY_READ_BUFFER, None);
                gl.bind_buffer(glow::COPY_WRITE_BUFFER, None);
            },
            com::Command::CopyBufferToTexture {
                src_buffer,
                dst_texture,
                texture_target,
                texture_format,
                pixel_type,
                ref data,
            } => unsafe {
                // TODO: Fix active texture
                assert_eq!(data.image_offset.z, 0);

                let gl = &self.share.context;

                gl.active_texture(glow::TEXTURE0);
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(src_buffer));

                match texture_target {
                    glow::TEXTURE_2D => {
                        gl.bind_texture(glow::TEXTURE_2D, Some(dst_texture));
                        gl.tex_sub_image_2d_pixel_buffer_offset(
                            glow::TEXTURE_2D,
                            data.image_layers.level as _,
                            data.image_offset.x,
                            data.image_offset.y,
                            data.image_extent.width as _,
                            data.image_extent.height as _,
                            texture_format,
                            pixel_type,
                            data.buffer_offset as i32,
                        );
                    }
                    glow::TEXTURE_2D_ARRAY => {
                        gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(dst_texture));
                        gl.tex_sub_image_3d_pixel_buffer_offset(
                            glow::TEXTURE_2D_ARRAY,
                            data.image_layers.level as _,
                            data.image_offset.x,
                            data.image_offset.y,
                            data.image_layers.layers.start as i32,
                            data.image_extent.width as _,
                            data.image_extent.height as _,
                            data.image_layers.layers.end as i32
                                - data.image_layers.layers.start as i32,
                            texture_format,
                            pixel_type,
                            data.buffer_offset as i32,
                        );
                    }
                    _ => unimplemented!(),
                }

                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            },
            com::Command::CopyBufferToRenderbuffer(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyTextureToBuffer {
                src_texture,
                texture_target,
                texture_format,
                pixel_type,
                dst_buffer,
                ref data,
            } => unsafe {
                // TODO: Fix active texture
                // TODO: handle partial copies gracefully
                assert_eq!(data.image_offset, hal::image::Offset { x: 0, y: 0, z: 0 });
                assert_eq!(texture_target, glow::TEXTURE_2D);
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0);
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(dst_buffer));
                gl.bind_texture(glow::TEXTURE_2D, Some(src_texture));
                gl.get_tex_image_pixel_buffer_offset(
                    glow::TEXTURE_2D,
                    data.image_layers.level as _,
                    //data.image_offset.x,
                    //data.image_offset.y,
                    //data.image_extent.width as _,
                    //data.image_extent.height as _,
                    texture_format,
                    pixel_type,
                    data.buffer_offset as i32,
                );
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            },
            com::Command::CopyRenderbufferToBuffer(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyImageToTexture(..) => {
                unimplemented!() //TODO: use FBO
            }
            com::Command::CopyImageToRenderbuffer {
                src_image,
                dst_renderbuffer,
                dst_format,
                ref data,
            } => {
                let gl = &self.share.context;

                if data.src_subresource.aspects != hal::format::Aspects::COLOR
                    || data.dst_subresource.aspects != hal::format::Aspects::COLOR
                {
                    unimplemented!()
                }

                match src_image {
                    native::ImageKind::Texture { .. } => unimplemented!(),
                    native::ImageKind::Renderbuffer {
                        renderbuffer: src_renderbuffer,
                        format: src_format,
                    } => {
                        if src_format != dst_format {
                            unimplemented!()
                        }

                        unsafe {
                            let src_fbo = gl.create_framebuffer().unwrap();
                            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(src_fbo));
                            gl.framebuffer_renderbuffer(
                                glow::READ_FRAMEBUFFER,
                                glow::COLOR_ATTACHMENT0,
                                glow::RENDERBUFFER,
                                Some(src_renderbuffer),
                            );

                            let dst_fbo = gl.create_framebuffer().unwrap();
                            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(dst_fbo));
                            gl.framebuffer_renderbuffer(
                                glow::DRAW_FRAMEBUFFER,
                                glow::COLOR_ATTACHMENT0,
                                glow::RENDERBUFFER,
                                Some(dst_renderbuffer),
                            );

                            gl.blit_framebuffer(
                                data.src_offset.x,
                                data.src_offset.y,
                                data.src_offset.x + data.extent.width as i32,
                                data.src_offset.y + data.extent.height as i32,
                                data.dst_offset.x,
                                data.dst_offset.y,
                                data.dst_offset.x + data.extent.width as i32,
                                data.dst_offset.y + data.extent.height as i32,
                                glow::COLOR_BUFFER_BIT,
                                glow::NEAREST,
                            );

                            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

                            gl.delete_framebuffer(src_fbo);
                            gl.delete_framebuffer(dst_fbo);
                        }
                    }
                }
            }
            com::Command::BindBufferRange(target, index, buffer, offset, size) => unsafe {
                let gl = &self.share.context;
                gl.bind_buffer_range(target, index, Some(buffer), offset, size);
            },
            com::Command::BindTexture(index, texture, textype) => unsafe {
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0 + index);
                gl.bind_texture(textype, Some(texture));
            },
            com::Command::BindSampler(index, sampler) => unsafe {
                let gl = &self.share.context;
                gl.bind_sampler(index, Some(sampler));
            },
            com::Command::SetTextureSamplerSettings(index, texture, textype, ref sinfo) => unsafe {
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0 + index);
                gl.bind_texture(textype, Some(texture));

                // TODO: Optimization: only change texture properties that have changed.
                device::set_sampler_info(
                    &sinfo,
                    &self.features,
                    &self.share.legacy_features,
                    |a, b| gl.tex_parameter_f32(textype, a, b),
                    |a, b| gl.tex_parameter_f32_slice(textype, a, &b),
                    |a, b| gl.tex_parameter_i32(textype, a, b),
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
            for i in 0..hal::MAX_COLOR_TARGETS {
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
            com::Command::BindUniform {
                ref uniform,
                buffer,
            } => {
                let gl = &self.share.context;

                unsafe {
                    match uniform.utype {
                        glow::FLOAT => {
                            let data = Self::get::<f32>(data_buf, buffer)[0];
                            gl.uniform_1_f32(Some((*uniform.location).clone()), data);
                        }
                        glow::FLOAT_VEC2 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[f32; 2]>(data_buf, buffer)[0];
                            gl.uniform_2_f32_slice(Some((*uniform.location).clone()), &mut data);
                        }
                        glow::FLOAT_VEC3 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[f32; 3]>(data_buf, buffer)[0];
                            gl.uniform_3_f32_slice(Some((*uniform.location).clone()), &mut data);
                        }
                        glow::FLOAT_VEC4 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[f32; 4]>(data_buf, buffer)[0];
                            gl.uniform_4_f32_slice(Some((*uniform.location).clone()), &mut data);
                        }
                        glow::INT => {
                            let data = Self::get::<i32>(data_buf, buffer)[0];
                            gl.uniform_1_i32(Some((*uniform.location).clone()), data);
                        }
                        glow::INT_VEC2 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[i32; 2]>(data_buf, buffer)[0];
                            gl.uniform_2_i32_slice(Some((*uniform.location).clone()), &mut data);
                        }
                        glow::INT_VEC3 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[i32; 3]>(data_buf, buffer)[0];
                            gl.uniform_3_i32_slice(Some((*uniform.location).clone()), &mut data);
                        }
                        glow::INT_VEC4 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[i32; 4]>(data_buf, buffer)[0];
                            gl.uniform_4_i32_slice(Some((*uniform.location).clone()), &mut data);
                        }
                        glow::FLOAT_MAT2 => {
                            let data = Self::get::<[f32; 4]>(data_buf, buffer)[0];
                            gl.uniform_matrix_2_f32_slice(
                                Some((*uniform.location).clone()),
                                false,
                                &data,
                            );
                        }
                        glow::FLOAT_MAT3 => {
                            let data = Self::get::<[f32; 9]>(data_buf, buffer)[0];
                            gl.uniform_matrix_3_f32_slice(
                                Some((*uniform.location).clone()),
                                false,
                                &data,
                            );
                        }
                        glow::FLOAT_MAT4 => {
                            let data = Self::get::<[f32; 16]>(data_buf, buffer)[0];
                            gl.uniform_matrix_4_f32_slice(
                                Some((*uniform.location).clone()),
                                false,
                                &data,
                            );
                        }
                        _ => panic!("Unsupported uniform datatype!"),
                    }
                }
            }
            com::Command::BindRasterizer { rasterizer } => {
                use hal::pso::FrontFace::*;
                use hal::pso::PolygonMode::*;

                let gl = &self.share.context;

                unsafe {
                    gl.front_face(match rasterizer.front_face {
                        Clockwise => glow::CW,
                        CounterClockwise => glow::CCW,
                    })
                };

                if !rasterizer.cull_face.is_empty() {
                    unsafe {
                        gl.enable(glow::CULL_FACE);
                        gl.cull_face(match rasterizer.cull_face {
                            hal::pso::Face::FRONT => glow::FRONT,
                            hal::pso::Face::BACK => glow::BACK,
                            _ => glow::FRONT_AND_BACK,
                        });
                    }
                } else {
                    unsafe {
                        gl.disable(glow::CULL_FACE);
                    }
                }

                let (gl_draw, gl_offset) = match rasterizer.polygon_mode {
                    Point => (glow::POINT, glow::POLYGON_OFFSET_POINT),
                    Line => (glow::LINE, glow::POLYGON_OFFSET_LINE),
                    Fill => (glow::FILL, glow::POLYGON_OFFSET_FILL),
                };

                if let hal::pso::State::Static(w) = rasterizer.line_width {
                    unsafe { gl.line_width(w) };
                }

                unsafe { gl.polygon_mode(glow::FRONT_AND_BACK, gl_draw) };

                match rasterizer.depth_bias {
                    Some(hal::pso::State::Static(bias)) => unsafe {
                        gl.enable(gl_offset);
                        gl.polygon_offset(bias.slope_factor as _, bias.const_factor as _);
                    },
                    _ => unsafe { gl.disable(gl_offset) },
                }

                if !self.share.info.is_webgl() && !self.share.info.version.is_embedded {
                    match false {
                        //TODO
                        true => unsafe { gl.enable(glow::MULTISAMPLE) },
                        false => unsafe { gl.disable(glow::MULTISAMPLE) },
                    }
                }
            }
            com::Command::BindDepth(depth_fun) => {
                use hal::pso::Comparison::*;

                let gl = &self.share.context;

                match depth_fun {
                    Some(depth_fun) => unsafe {
                        gl.enable(glow::DEPTH_TEST);

                        let cmp = match depth_fun {
                            Never => glow::NEVER,
                            Less => glow::LESS,
                            LessEqual => glow::LEQUAL,
                            Equal => glow::EQUAL,
                            GreaterEqual => glow::GEQUAL,
                            Greater => glow::GREATER,
                            NotEqual => glow::NOTEQUAL,
                            Always => glow::ALWAYS,
                        };

                        gl.depth_func(cmp);
                    },
                    None => unsafe {
                        gl.disable(glow::DEPTH_TEST);
                    },
                }
            }
            com::Command::SetColorMask(slot, mask) => unsafe {
                use hal::pso::ColorMask as Cm;
                if let Some(slot) = slot {
                    self.share.context.color_mask_draw_buffer(
                        slot,
                        mask.contains(Cm::RED) as _,
                        mask.contains(Cm::GREEN) as _,
                        mask.contains(Cm::BLUE) as _,
                        mask.contains(Cm::ALPHA) as _,
                    );
                } else {
                    self.share.context.color_mask(
                        mask.contains(Cm::RED) as _,
                        mask.contains(Cm::GREEN) as _,
                        mask.contains(Cm::BLUE) as _,
                        mask.contains(Cm::ALPHA) as _,
                    );
                }
            },
            com::Command::SetDepthMask(write) => unsafe {
                self.share.context.depth_mask(write);
            },
            com::Command::SetStencilMask(value) => unsafe {
                self.share.context.stencil_mask(value);
            },
            com::Command::SetStencilMaskSeparate(values) => unsafe {
                self.share
                    .context
                    .stencil_mask_separate(glow::FRONT, values.front);
                self.share
                    .context
                    .stencil_mask_separate(glow::BACK, values.back);
            }, /*
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
}

impl hal::queue::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        submit_info: hal::queue::Submission<Ic, Iw, Is>,
        fence: Option<&native::Fence>,
    ) where
        T: 'a + Borrow<com::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, hal::pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        use crate::pool::BufferMemory;
        {
            for buf in submit_info.command_buffers {
                let cb = &buf.borrow().data;
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

        if let Some(fence) = fence {
            if self.share.private_caps.sync {
                fence.0.set(native::FenceInner::Pending(Some(
                    self.share
                        .context
                        .fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0)
                        .unwrap(),
                )));
            } else {
                self.share.context.flush();
                fence.0.set(native::FenceInner::Idle { signaled: true });
            }
        }
    }

    unsafe fn present<'a, W, Is, S, Iw>(
        &mut self,
        swapchains: Is,
        _wait_semaphores: Iw,
    ) -> Result<Option<hal::window::Suboptimal>, hal::window::PresentError>
    where
        W: 'a + Borrow<crate::Swapchain>,
        Is: IntoIterator<Item = (&'a W, hal::window::SwapImageIndex)>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = &'a S>,
    {
        for (swapchain, index) in swapchains {
            self.present_by_copy(swapchain.borrow(), index);
        }

        #[cfg(wgl)]
        self.share.instance_context.make_current();

        Ok(None)
    }

    unsafe fn present_surface(
        &mut self,
        surface: &mut Surface,
        _image: native::ImageView,
        _wait_semaphore: Option<&native::Semaphore>,
    ) -> Result<Option<hal::window::Suboptimal>, hal::window::PresentError> {
        let swapchain = surface
            .swapchain
            .as_ref()
            .expect("No swapchain is configured!");
        self.present_by_copy(swapchain, 0);

        #[cfg(wgl)]
        self.share.instance_context.make_current();

        Ok(None)
    }

    fn wait_idle(&self) -> Result<(), hal::device::OutOfMemory> {
        unsafe {
            self.share.context.finish();
        }
        Ok(())
    }
}
