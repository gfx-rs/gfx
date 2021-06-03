use crate::{
    command as com, device, info::LegacyFeatures, native, state, Backend, Device, GlContext, Share,
    Starc, Surface, MAX_COLOR_ATTACHMENTS,
};

use arrayvec::ArrayVec;
use glow::HasContext;

use std::{mem, slice};

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

#[derive(Debug)]
pub struct Queue {
    pub(crate) share: Starc<Share>,
    features: hal::Features,
    vao: Option<native::VertexArray>,
    state: State,
    fill_buffer: native::RawBuffer,
    fill_data: Box<[u32]>,
}

const FILL_DATA_WORDS: usize = 16 << 10;

impl Queue {
    /// Create a new command queue.
    pub(crate) fn new(
        share: &Starc<Share>,
        features: hal::Features,
        vao: Option<native::VertexArray>,
    ) -> Self {
        let gl = &share.context;
        let fill_buffer = unsafe {
            let buffer = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::COPY_READ_BUFFER, Some(buffer));
            gl.buffer_data_size(
                glow::COPY_READ_BUFFER,
                FILL_DATA_WORDS as i32 * 4,
                glow::STREAM_DRAW,
            );
            gl.bind_buffer(glow::COPY_READ_BUFFER, None);
            buffer
        };
        Queue {
            share: share.clone(),
            features,
            vao,
            state: State::new(),
            fill_buffer,
            fill_data: vec![0; FILL_DATA_WORDS].into_boxed_slice(),
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
            C::Float => (glow::ZERO, glow::HALF_FLOAT, glow),
            C::Srgb => {
                log::error!("Unsupported Srgb channel type");
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
                log::error!("Unsupported element type: {:?}", bel.elem.format.0);
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
            log::error!("Instanced arrays are not supported");
        }
    }
    */

    fn bind_target(&mut self, point: u32, attachment: u32, view: &native::ImageView) {
        Device::bind_target(&self.share.context, point, attachment, view)
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

        // Reset viewports && scissors
        unsafe {
            gl.viewport(0, 0, 0, 0);
            gl.depth_range_f32(0.0, 1.0);
            gl.scissor(0, 0, 0, 0);
        };
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
                        log::error!(
                            "Instanced draw calls with non-zero base instance are not supported"
                        );
                    }
                } else {
                    log::error!("Instanced draw calls are not supported");
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
                let caveats = &self.share.public_caps.performance_caveats;

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
                        log::error!("Base vertex with indexed drawing not supported");
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
                        log::error!("Base vertex with instanced indexed drawing is not supported");
                    } else if caveats
                        .contains(hal::PerformanceCaveats::BASE_VERTEX_INSTANCE_DRAWING)
                    {
                        //TODO: this is supposed to be a workaround, not an error
                        log::error!(
                            "Instance bases with instanced indexed drawing is not supported"
                        );
                    } else {
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
                    }
                } else {
                    log::error!("Instanced indexed drawing is not supported");
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
                assert!(
                    0 < num_viewports
                        && num_viewports <= self.share.public_caps.limits.max_viewports
                );

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
                            log::trace!("Depth ranges with f64 precision are not supported, falling back to f32");
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
                assert!(
                    0 < num_scissors && num_scissors <= self.share.public_caps.limits.max_viewports
                );

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
            com::Command::BindFramebuffer {
                target,
                framebuffer,
                ref colors,
                ref depth_stencil,
            } => {
                let gl = &self.share.context;
                unsafe { gl.bind_framebuffer(target, Some(framebuffer)) };
                for (i, view) in colors.iter().enumerate() {
                    self.bind_target(target, glow::COLOR_ATTACHMENT0 + i as u32, view);
                }
                if let Some(ref view) = *depth_stencil {
                    let aspects = view.aspects();
                    let attachment = if aspects == hal::format::Aspects::DEPTH {
                        glow::DEPTH_ATTACHMENT
                    } else if aspects == hal::format::Aspects::STENCIL {
                        glow::STENCIL_ATTACHMENT
                    } else {
                        glow::DEPTH_STENCIL_ATTACHMENT
                    };
                    self.bind_target(target, attachment, view);
                }
            }
            com::Command::FillBuffer(buffer, ref range, value) => {
                //Note: buffers with `DYNAMIC_STORAGE_BIT` can't be uploaded to directly.
                // And we expect the target buffers to be on GPU, where we assign this flag.

                let total_size = (range.end - range.start) as i32;
                let temp_size = (total_size as usize / 4).min(FILL_DATA_WORDS);
                let mut dst_offset = range.start as i32;
                for v in self.fill_data[..temp_size].iter_mut() {
                    *v = value;
                }

                let gl = &self.share.context;
                unsafe {
                    gl.bind_buffer(glow::COPY_READ_BUFFER, Some(self.fill_buffer));
                    gl.buffer_sub_data_u8_slice(
                        glow::COPY_READ_BUFFER,
                        0,
                        slice::from_raw_parts(self.fill_data.as_ptr() as *const u8, temp_size * 4),
                    );
                    gl.bind_buffer(glow::COPY_WRITE_BUFFER, Some(buffer));

                    while dst_offset < range.end as i32 {
                        let copy_size = (temp_size as i32 * 4).min(range.end as i32 - dst_offset);
                        gl.copy_buffer_sub_data(
                            glow::COPY_READ_BUFFER,
                            glow::COPY_WRITE_BUFFER,
                            0,
                            dst_offset,
                            copy_size,
                        );
                        dst_offset += copy_size;
                    }

                    gl.bind_buffer(glow::COPY_READ_BUFFER, None);
                    gl.bind_buffer(glow::COPY_WRITE_BUFFER, None);
                }
            }
            com::Command::SetDrawColorBuffers(ref indices) => {
                let gl_indices = indices
                    .iter()
                    .map(|&i| glow::COLOR_ATTACHMENT0 + i as u32)
                    .collect::<ArrayVec<[_; MAX_COLOR_ATTACHMENTS]>>();
                unsafe { self.share.context.draw_buffers(&gl_indices) };
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
                    log::warn!("Draw buffers are not supported");
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
                    log::error!("Binding attribute with instanced input rate is not supported");
                }

                gl.enable_vertex_attrib_array(location);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            },
            /*
            com::Command::UnbindAttribute(ref attribute) => unsafe {
                self.share.context.DisableVertexAttribArray(attribute.location);
            }*/
            com::Command::CopyBufferToBuffer {
                src_buffer,
                dst_buffer,
                src_target: _,
                dst_target,
                data,
            } => unsafe {
                let is_index_buffer_only_element_dst =
                    !self.share.private_caps.index_buffer_role_change
                        && dst_target == glow::ELEMENT_ARRAY_BUFFER;

                let copy_src_target = glow::COPY_READ_BUFFER;
                // WebGL not allowed to copy data from other targets to element buffer and can't copy element data to other buffers
                let copy_dst_target = if is_index_buffer_only_element_dst {
                    glow::ELEMENT_ARRAY_BUFFER
                } else {
                    glow::COPY_WRITE_BUFFER
                };
                let gl = &self.share.context;

                gl.bind_buffer(copy_src_target, Some(src_buffer));
                gl.bind_buffer(copy_dst_target, Some(dst_buffer));

                if is_index_buffer_only_element_dst {
                    let mut buffer_data = vec![0; data.size as usize];
                    gl.get_buffer_sub_data(copy_src_target, data.src as i32, &mut buffer_data);
                    gl.buffer_sub_data_u8_slice(copy_dst_target, data.dst as i32, &buffer_data);
                } else {
                    gl.copy_buffer_sub_data(
                        copy_src_target,
                        copy_dst_target,
                        data.src as _,
                        data.dst as _,
                        data.size as _,
                    );
                }

                gl.bind_buffer(copy_src_target, None);

                if is_index_buffer_only_element_dst {
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, self.state.index_buffer);
                } else {
                    gl.bind_buffer(copy_dst_target, None);
                }
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
                gl.bind_texture(texture_target, Some(dst_texture));

                match texture_target {
                    glow::TEXTURE_2D => {
                        gl.tex_sub_image_2d(
                            texture_target,
                            data.image_layers.level as _,
                            data.image_offset.x,
                            data.image_offset.y,
                            data.image_extent.width as _,
                            data.image_extent.height as _,
                            texture_format,
                            pixel_type,
                            glow::PixelUnpackData::BufferOffset(data.buffer_offset as u32),
                        );
                    }
                    glow::TEXTURE_2D_ARRAY | glow::TEXTURE_3D => {
                        gl.tex_sub_image_3d(
                            texture_target,
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
                            glow::PixelUnpackData::BufferOffset(data.buffer_offset as u32),
                        );
                    }
                    glow::TEXTURE_CUBE_MAP => {
                        let components = match texture_format {
                            glow::RED
                            | glow::RED_INTEGER
                            | glow::DEPTH_COMPONENT
                            | glow::DEPTH_STENCIL => 1,
                            glow::RG | glow::RG_INTEGER => 2,
                            glow::RGB | glow::RGB_INTEGER => 3,
                            glow::RGBA | glow::BGRA | glow::RGBA_INTEGER => 4,
                            _ => unreachable!(),
                        };

                        let component_size = match pixel_type {
                            glow::BYTE | glow::UNSIGNED_BYTE => 1,
                            glow::SHORT | glow::UNSIGNED_SHORT | glow::HALF_FLOAT => 2,
                            glow::INT
                            | glow::UNSIGNED_INT
                            | glow::UNSIGNED_NORMALIZED
                            | glow::FLOAT => 4,
                            _ => unreachable!(),
                        };

                        let mut buffer_offset = data.buffer_offset as u32;
                        let layer_size =
                            data.buffer_width * data.buffer_height * component_size * components;

                        let faces_range = data.image_layers.layers.start as usize
                            ..data.image_layers.layers.end as usize;
                        for &face in &[
                            glow::TEXTURE_CUBE_MAP_POSITIVE_X,
                            glow::TEXTURE_CUBE_MAP_NEGATIVE_X,
                            glow::TEXTURE_CUBE_MAP_POSITIVE_Y,
                            glow::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                            glow::TEXTURE_CUBE_MAP_POSITIVE_Z,
                            glow::TEXTURE_CUBE_MAP_NEGATIVE_Z,
                        ][faces_range]
                        {
                            gl.tex_sub_image_2d(
                                face,
                                data.image_layers.level as _,
                                data.image_offset.x,
                                data.image_offset.y,
                                data.image_extent.width as _,
                                data.image_extent.height as _,
                                texture_format,
                                pixel_type,
                                glow::PixelUnpackData::BufferOffset(buffer_offset),
                            );
                            buffer_offset += layer_size;
                        }
                    }
                    _ => unimplemented!(),
                }

                gl.bind_texture(texture_target, None);
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            },
            com::Command::CopyBufferToRenderbuffer(..) => {
                log::error!("CopyBufferToRenderbuffer is not implemented");
            }
            com::Command::CopyTextureToBuffer {
                src_texture,
                texture_target,
                texture_format,
                pixel_type,
                dst_buffer,
                ref data,
            } => {
                if self.share.private_caps.get_tex_image {
                    // TODO: Fix active texture
                    // TODO: handle partial copies gracefully
                    assert_eq!(data.image_offset, hal::image::Offset { x: 0, y: 0, z: 0 });
                    assert_eq!(texture_target, glow::TEXTURE_2D);
                    let gl = &self.share.context;
                    unsafe {
                        gl.active_texture(glow::TEXTURE0);
                        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(dst_buffer));
                        gl.bind_texture(glow::TEXTURE_2D, Some(src_texture));
                        gl.get_tex_image(
                            glow::TEXTURE_2D,
                            data.image_layers.level as _,
                            //data.image_offset.x,
                            //data.image_offset.y,
                            //data.image_extent.width as _,
                            //data.image_extent.height as _,
                            texture_format,
                            pixel_type,
                            glow::PixelPackData::BufferOffset(data.buffer_offset as u32),
                        );
                        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
                        gl.bind_texture(glow::TEXTURE_2D, None);
                    }
                } else {
                    //TODO: use FBO
                    log::error!("CopyTextureToBuffer is not implemented on GLES");
                }
            }
            com::Command::CopyRenderbufferToBuffer(..) => {
                //TODO: use FBO
                log::error!("CopyRenderbufferToBuffer is not implemented");
            }
            com::Command::CopyImageToTexture(..) => {
                //TODO: use FBO
                log::error!("CopyImageToTexture is not implemented");
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
                    native::ImageType::Texture { .. } => unimplemented!(),
                    native::ImageType::Renderbuffer {
                        raw: src_renderbuffer,
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
            com::Command::SetTextureSamplerSettings(index, textype, ref sinfo) => unsafe {
                let gl = &self.share.context;
                gl.active_texture(glow::TEXTURE0 + index);
                // TODO: Optimization: only change texture properties that have changed.
                device::set_sampler_info(
                    &sinfo,
                    &self.features,
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
            log::error!("Trying to bind a sampler to slot {}, when sampler objects are not supported, and no texture is bound there", slot);
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
                            gl.uniform_1_f32(Some(&(*uniform.location).clone()), data);
                        }
                        glow::FLOAT_VEC2 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[f32; 2]>(data_buf, buffer)[0];
                            gl.uniform_2_f32_slice(Some(&(*uniform.location).clone()), &mut data);
                        }
                        glow::FLOAT_VEC3 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[f32; 3]>(data_buf, buffer)[0];
                            gl.uniform_3_f32_slice(Some(&(*uniform.location).clone()), &mut data);
                        }
                        glow::FLOAT_VEC4 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[f32; 4]>(data_buf, buffer)[0];
                            gl.uniform_4_f32_slice(Some(&(*uniform.location).clone()), &mut data);
                        }
                        glow::INT => {
                            let data = Self::get::<i32>(data_buf, buffer)[0];
                            gl.uniform_1_i32(Some(&(*uniform.location).clone()), data);
                        }
                        glow::INT_VEC2 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[i32; 2]>(data_buf, buffer)[0];
                            gl.uniform_2_i32_slice(Some(&(*uniform.location).clone()), &mut data);
                        }
                        glow::INT_VEC3 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[i32; 3]>(data_buf, buffer)[0];
                            gl.uniform_3_i32_slice(Some(&(*uniform.location).clone()), &mut data);
                        }
                        glow::INT_VEC4 => {
                            // TODO: Remove`mut`
                            let mut data = Self::get::<[i32; 4]>(data_buf, buffer)[0];
                            gl.uniform_4_i32_slice(Some(&(*uniform.location).clone()), &mut data);
                        }
                        glow::FLOAT_MAT2 => {
                            let data = Self::get::<[f32; 4]>(data_buf, buffer)[0];
                            gl.uniform_matrix_2_f32_slice(
                                Some(&(*uniform.location).clone()),
                                false,
                                &data,
                            );
                        }
                        glow::FLOAT_MAT3 => {
                            let data = Self::get::<[f32; 9]>(data_buf, buffer)[0];
                            gl.uniform_matrix_3_f32_slice(
                                Some(&(*uniform.location).clone()),
                                false,
                                &data,
                            );
                        }
                        glow::FLOAT_MAT4 => {
                            let data = Self::get::<[f32; 16]>(data_buf, buffer)[0];
                            gl.uniform_matrix_4_f32_slice(
                                Some(&(*uniform.location).clone()),
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

                let (_gl_draw, gl_offset) = match rasterizer.polygon_mode {
                    Point => (glow::POINT, glow::POLYGON_OFFSET_POINT),
                    Line => (glow::LINE, glow::POLYGON_OFFSET_LINE),
                    Fill => (glow::FILL, glow::POLYGON_OFFSET_FILL),
                };

                if let hal::pso::State::Static(w) = rasterizer.line_width {
                    if w != 1.0 {
                        // Default value already 1.0
                        unsafe { gl.line_width(w) };
                    }
                }

                //TODO: this is not available in GLES
                //unsafe { gl.polygon_mode(glow::FRONT_AND_BACK, gl_draw) };

                match rasterizer.depth_bias {
                    Some(hal::pso::State::Static(bias)) => unsafe {
                        gl.enable(gl_offset);
                        gl.polygon_offset(bias.slope_factor as _, bias.const_factor as _);
                    },
                    _ => unsafe { gl.disable(gl_offset) },
                }

                if !self.share.info.version.is_embedded {
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
                if let (true, Some(slot)) = (self.share.private_caps.per_slot_color_mask, slot) {
                    self.share.context.color_mask_draw_buffer(
                        slot,
                        mask.contains(Cm::RED) as _,
                        mask.contains(Cm::GREEN) as _,
                        mask.contains(Cm::BLUE) as _,
                        mask.contains(Cm::ALPHA) as _,
                    );
                } else {
                    if slot.is_some() {
                        // TODO: the generator of these commands should coalesce identical masks to prevent this warning
                        //       as much as is possible.
                        log::warn!("GLES and WebGL do not support per-target color masks. Falling back on global mask.");
                    }
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
            },
            com::Command::MemoryBarrier(mask) => {
                if self.share.private_caps.memory_barrier {
                    unsafe {
                        self.share.context.memory_barrier(mask);
                    }
                }
            }
        }
        if let Err(err) = self.share.check() {
            panic!("Error {:?} executing command: {:?}", err, cmd)
        }
    }
}

impl hal::queue::Queue<Backend> for Queue {
    unsafe fn submit<'a, Ic, Iw, Is>(
        &mut self,
        command_buffers: Ic,
        _wait_semaphores: Iw,
        _signal_semaphores: Is,
        fence: Option<&mut native::Fence>,
    ) where
        Ic: Iterator<Item = &'a com::CommandBuffer>,
        Iw: Iterator<Item = (&'a native::Semaphore, hal::pso::PipelineStage)>,
        Is: Iterator<Item = &'a native::Semaphore>,
    {
        use crate::pool::BufferMemory;
        {
            for cmd_buf in command_buffers {
                let cb = &cmd_buf.data;
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
                    log::trace!("Execute command:{:?}", com);
                    self.process(com, &buffer.data);
                }
            }
        }

        if let Some(fence) = fence {
            *fence = if self.share.private_caps.sync {
                native::Fence::Pending(
                    self.share
                        .context
                        .fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0)
                        .unwrap(),
                )
            } else {
                self.share.context.flush();
                native::Fence::Idle { signaled: true }
            }
        }
    }

    unsafe fn present(
        &mut self,
        surface: &mut Surface,
        image: native::SwapchainImage,
        _wait_semaphore: Option<&mut native::Semaphore>,
    ) -> Result<Option<hal::window::Suboptimal>, hal::window::PresentError> {
        surface.present(image, &self.share.context)
    }

    fn wait_idle(&mut self) -> Result<(), hal::device::OutOfMemory> {
        unsafe {
            self.share.context.finish();
        }
        Ok(())
    }

    fn timestamp_period(&self) -> f32 {
        1.0
    }
}
