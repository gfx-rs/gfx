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

//! Graphics commands encoder.

#![deny(missing_docs)]

use std::mem;
use draw_state::target::{ClearData, ColorValue, Depth, Mask, Mirror, Rect, Stencil};

use gfx_core as device;
use gfx_core::Resources;
use gfx_core::{format, handle};
use gfx_core::attrib::IntSize;
use gfx_core::draw::{Access, Gamma, Target};
use gfx_core::draw::{CommandBuffer, DataBuffer, InstanceOption};
use gfx_core::factory::{Factory, NotSupported};
use gfx_core::output::{Output, Plane};
use gfx_core::shade::{ProgramInfo, UniformValue};
use gfx_core::tex::Size;
use mesh;
use pso;
use shade::TextureParam;
use target;

/// An error occuring in surface blits.
#[derive(Clone, Debug, PartialEq)]
pub enum BlitError {
    /// The source doesn't have some of the requested planes.
    SourcePlanesMissing(Mask),
    /// The destination doesn't have some of the requested planes.
    DestinationPlanesMissing(Mask),
}

/// An error occuring in buffer/texture updates.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateError<T> {
    OutOfBounds {
        target: T,
        source: T,
    },
    UnitSizeMismatch {
        target: u8,
        source: u8,
    },
    UnitCountMismatch {
        target: usize,
        slice: usize,
    },
}


/// The internal state of the renderer.
/// This is used as a cache to eliminate redundant state changes.
struct RenderState<R: Resources> {
    frame_buffer: Option<handle::FrameBuffer<R>>,
    frame: target::Frame<R>,
    gamma: Gamma,
    index: Option<handle::RawBuffer<R>>,
}

impl<R: Resources> RenderState<R> {
    /// Generate the initial state matching `Device::reset_state`
    fn new() -> RenderState<R> {
        RenderState {
            frame_buffer: None,
            frame: target::Frame::empty(0,0),
            gamma: Gamma::Original,
            index: None,
        }
    }
}

/// Temporary parameter storage, used for shader activation.
pub struct ParamStorage<R: Resources> {
    /// uniform values to be provided
    pub uniforms: Vec<Option<UniformValue>>,
    /// uniform buffers to be provided
    pub blocks  : Vec<Option<handle::RawBuffer<R>>>,
    /// textures to be provided
    pub textures: Vec<Option<TextureParam<R>>>,
}

impl<R: Resources> ParamStorage<R> {
    /// Create an empty parameter storage.
    pub fn new() -> ParamStorage<R> {
        ParamStorage {
            uniforms: Vec::new(),
            blocks: Vec::new(),
            textures: Vec::new(),
        }
    }

    /// Reserve the exact slots needed for this program info.
    pub fn reserve(&mut self, pinfo: &ProgramInfo) {
        // clear
        self.uniforms.clear();
        self.blocks  .clear();
        self.textures.clear();
        // allocate
        self.uniforms.extend(pinfo.globals.iter().map(|_| None));
        self.blocks  .extend(pinfo.constant_buffers.iter().map(|_| None));
        self.textures.extend(pinfo.textures.iter().map(|_| None));
    }
}

/// Extension methods for the command buffer.
/// Useful when Renderer is borrowed, and we need to issue commands.
trait CommandBufferExt<R: Resources>: CommandBuffer<R> {
    /// Bind a plane to some target
    fn bind_target(&mut self, &mut handle::Manager<R>, Access, Target,
                   Option<&Plane<R>>);
}

impl<R: Resources, C: CommandBuffer<R>> CommandBufferExt<R> for C {
    fn bind_target(&mut self, handles: &mut handle::Manager<R>, access: Access,
                   to: Target, plane: Option<&Plane<R>>) {
        match plane {
            None => self.unbind_target(access, to),
            Some(&Plane::Surface(ref suf)) =>
                self.bind_target_surface(access, to, handles.ref_surface(suf).clone()),
            Some(&Plane::Texture(ref tex, level, layer)) =>
                self.bind_target_texture(access, to, handles.ref_texture(tex).clone(), level, layer),
        }
    }
}

/// Graphics commands encoder.
pub struct Encoder<R: Resources, C: CommandBuffer<R>> {
    command_buffer: C,
    data_buffer: DataBuffer,
    handles: handle::Manager<R>,
    common_array_buffer: Result<handle::ArrayBuffer<R>, NotSupported>,
    draw_frame_buffer: Result<handle::FrameBuffer<R>, NotSupported>,
    read_frame_buffer: Result<handle::FrameBuffer<R>, NotSupported>,
    render_state: RenderState<R>,
}

impl<R: Resources, C: CommandBuffer<R>> Encoder<R, C> {
    /// Create a new encoder using a factory.
    pub fn create<F>(factory: &mut F) -> Encoder<R, C> where
        F: Factory<R, CommandBuffer = C>
    {
        Encoder {
            command_buffer: factory.create_command_buffer(),
            data_buffer: DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: factory.create_array_buffer(),
            draw_frame_buffer: factory.create_frame_buffer(),
            read_frame_buffer: factory.create_frame_buffer(),
            render_state: RenderState::new(),
        }
    }

    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.command_buffer.clear();
        self.data_buffer.clear();
        self.handles.clear();
        self.render_state = RenderState::new();
    }

    /// Get command and data buffers to be submitted to the device.
    pub fn as_buffer<D>(&self) -> device::SubmitInfo<D> where
        D: device::Device<Resources=R, CommandBuffer=C> {
        device::SubmitInfo(&self.command_buffer, &self.data_buffer, &self.handles)
    }

    /// Clone the renderer shared data but ignore the commands.
    pub fn clone_empty(&self) -> Encoder<R, C> {
        Encoder {
            command_buffer: self.command_buffer.clone_empty(),
            data_buffer: DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: self.common_array_buffer.clone(),
            draw_frame_buffer: self.draw_frame_buffer.clone(),
            read_frame_buffer: self.read_frame_buffer.clone(),
            render_state: RenderState::new(),
        }
    }

    /// Clear the output with given `ClearData`.
    pub fn clear<O: Output<R>>(&mut self, data: ClearData, mask: Mask, output: &O) {
        let has_mask = output.get_mask();
        if has_mask.is_empty() {
            panic!("Clearing a frame without any attachments is not possible!
                    If you are using `Frame::empty` in place of a real output window,
                    please see https://github.com/gfx-rs/gfx-rs/pull/682");
        }
        debug_assert!(has_mask.contains(mask));
        self.bind_output(output);
        self.command_buffer.call_clear(data, mask);
    }

    /// Blit one frame onto another.
    pub fn blit<I: Output<R>, O: Output<R>>(&mut self,
                source: &I, source_rect: Rect,
                destination: &O, dest_rect: Rect,
                mirror: Mirror, mask: Mask)
                -> Result<(), BlitError>
    {
        if !source.get_mask().contains(mask) {
            let missing = mask - source.get_mask();
            return Err(BlitError::SourcePlanesMissing(missing))
        }
        if !destination.get_mask().contains(mask) {
            let missing = mask - destination.get_mask();
            return Err(BlitError::DestinationPlanesMissing(missing))
        }
        self.bind_output(destination);
        self.bind_pixel_input(source);
        self.command_buffer.call_blit(source_rect, dest_rect, mirror, mask);
        Ok(())
    }

    /// Update a buffer with a slice of data.
    pub fn update_buffer<T: Copy>(&mut self, buf: &handle::RawBuffer<R>,
                         data: &[T], offset_elements: usize)
                         -> Result<(), UpdateError<usize>>
    {
        if data.is_empty() {
            return Ok(())
        }
        let elem_size = mem::size_of::<T>();
        let offset_bytes = elem_size * offset_elements;
        let bound = data.len() * elem_size + offset_bytes;
        if bound <= buf.get_info().size {
            let pointer = self.data_buffer.add_vec(data);
            self.command_buffer.update_buffer(
                self.handles.ref_buffer(buf).clone(),
                pointer, offset_bytes);
            Ok(())
        } else {
            Err(UpdateError::OutOfBounds {
                target: bound,
                source: buf.get_info().size,
            })
        }
    }

    /// Update a buffer with a data struct.
    pub fn update_block<U, T: Copy>(&mut self, buf: &handle::Buffer<R, U>, data: &T)
                        -> Result<(), UpdateError<usize>>
    {
        let bound = mem::size_of::<T>();
        if bound <= buf.get_info().size {
            use gfx_core::factory::Phantom;
            let pointer = self.data_buffer.add_struct(data);
            self.command_buffer.update_buffer(
                self.handles.ref_buffer(buf.raw()).clone(),
                pointer, 0);
            Ok(())
        } else {
            Err(UpdateError::OutOfBounds {
                target: bound,
                source: buf.get_info().size,
            })
        }
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Copy>(&mut self, tex: &handle::Texture<R>,
                          img: device::tex::ImageInfo, data: &[T])
                          -> Result<(), UpdateError<[Size; 3]>>
    {
        if data.is_empty() {
            return Ok(())
        }

        let source_size = tex.get_info().format.get_size().unwrap_or(0);
        let target_size = mem::size_of::<T>() as u8;
        if source_size != target_size {
            return Err(UpdateError::UnitSizeMismatch {
                target: target_size,
                source: source_size,
            })
        }

        let target_count = img.get_texel_count();
        if target_count != data.len() {
            return Err(UpdateError::UnitCountMismatch {
                target: target_count,
                slice: data.len(),
            })
        }

        if !tex.get_info().contains(&img) {
            let (w, h, d, _) = tex.get_info().kind.get_dimensions();
            return Err(UpdateError::OutOfBounds {
                target: [
                    img.xoffset + img.width,
                    img.yoffset + img.height,
                    img.zoffset + img.depth,
                ],
                source: [w, h, d],
            })
        }

        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_texture(tex.get_info().kind,
            self.handles.ref_texture(tex).clone(), img, pointer);
        Ok(())
    }

    fn bind_output<O: Output<R>>(&mut self, output: &O) {
        let (width, height) = output.get_size();
        if self.render_state.frame.width != width ||
                self.render_state.frame.height != height {
            self.command_buffer.set_viewport(Rect {x: 0, y: 0, w: width, h: height});
            self.render_state.frame.width = width;
            self.render_state.frame.height = height;
        }
        let gamma = output.get_gamma();
        let change_gamma = self.render_state.gamma != gamma;

        match output.get_handle() {
            Some(ref handle) => {
                if self.render_state.frame_buffer.as_ref() != Some(handle) || change_gamma {
                    self.command_buffer.bind_frame_buffer(Access::Draw,
                        self.handles.ref_frame_buffer(handle).clone(),
                        gamma);
                    self.render_state.frame_buffer = Some((*handle).clone());
                    self.render_state.gamma = gamma;
                }
            },
            None => {
                let draw_fbo = self.draw_frame_buffer.as_ref().ok().expect(
                    "Unable to use off-screen draw targets: not supported by the backend");
                if self.render_state.frame_buffer.as_ref() != Some(draw_fbo) || change_gamma {
                    self.command_buffer.bind_frame_buffer(Access::Draw,
                        self.handles.ref_frame_buffer(draw_fbo).clone(),
                        gamma);
                    self.render_state.frame_buffer = Some(draw_fbo.clone());
                    self.render_state.gamma = gamma;
                }
                let colors = output.get_colors();
                // cut off excess color planes
                for (i, _) in self.render_state.frame.colors.iter().enumerate()
                                    .skip(colors.len()) {
                    self.command_buffer.unbind_target(Access::Draw, Target::Color(i as u8));
                }
                self.render_state.frame.colors.truncate(colors.len());
                // bind intersecting subsets
                for (i, (cur, new)) in self.render_state.frame.colors.iter_mut()
                                           .zip(colors.iter()).enumerate() {
                    if *cur != *new {
                        self.command_buffer.bind_target(&mut self.handles,
                            Access::Draw, Target::Color(i as u8), Some(new));
                        *cur = new.clone();
                    }
                }
                // activate the color targets that were just bound
                self.command_buffer.set_draw_color_buffers(colors.len() as device::ColorSlot);
                // append new planes
                for (i, new) in colors.iter().enumerate()
                                      .skip(self.render_state.frame.colors.len()) {
                    self.command_buffer.bind_target(&mut self.handles,
                        Access::Draw, Target::Color(i as u8), Some(new));
                    self.render_state.frame.colors.push(new.clone());
                }
                // set depth
                let depth = output.get_depth();
                if self.render_state.frame.depth.as_ref() != depth {
                    self.command_buffer.bind_target(&mut self.handles,
                        Access::Draw, Target::Depth, depth);
                    self.render_state.frame.depth = depth.map(|p| p.clone());
                }
                // set stencil
                let stencil = output.get_stencil();
                if self.render_state.frame.stencil.as_ref() != stencil {
                    self.command_buffer.bind_target(&mut self.handles,
                        Access::Draw, Target::Stencil, stencil);
                    self.render_state.frame.stencil = stencil.map(|p| p.clone());
                }
            },
        }
    }

    fn bind_pixel_input<I: Output<R>>(&mut self, input: &I) {
        // bind input
        if let Some(ref handle) = input.get_handle() {
            self.command_buffer.bind_frame_buffer(Access::Read,
                self.handles.ref_frame_buffer(handle).clone(),
                Gamma::Original);
        }else if let Ok(ref fbo) = self.read_frame_buffer {
            self.command_buffer.bind_frame_buffer(Access::Read,
                self.handles.ref_frame_buffer(fbo).clone(),
                Gamma::Original);
        }else {
            panic!("Unable to use off-screen read targets: not supported by the backend");
        }
        // color
        match input.get_colors().first() {
            Some(ref color) => {
                self.command_buffer.bind_target(&mut self.handles,
                    Access::Read, Target::Color(0), Some(color));
            },
            None => {
                self.command_buffer.unbind_target(Access::Read, Target::Color(0));
            },
        }
        // depth/stencil
        self.command_buffer.bind_target(&mut self.handles,
            Access::Read, Target::Depth, input.get_depth());
        self.command_buffer.bind_target(&mut self.handles,
            Access::Read, Target::Stencil, input.get_stencil());
    }

    fn draw_indexed<T>(&mut self, buf: &handle::Buffer<R, T>, format: IntSize,
                     slice: &mesh::Slice<R>, base: device::VertexCount,
                     instances: InstanceOption) {
        use gfx_core::factory::Phantom;
        if self.render_state.index.as_ref() != Some(buf.raw()) {
            self.render_state.index = Some(buf.raw().clone());
            self.command_buffer.bind_index(self.handles.ref_buffer(buf.raw()).clone());
        }
        self.command_buffer.call_draw_indexed(format,
            slice.start, slice.end - slice.start, base, instances);
    }

    fn draw_slice(&mut self, slice: &mesh::Slice<R>, instances: InstanceOption) {
        match slice.kind {
            mesh::SliceKind::Vertex => self.command_buffer.call_draw(
                slice.start, slice.end - slice.start, instances),
            mesh::SliceKind::Index8(ref buf, base) =>
                self.draw_indexed(buf, IntSize::U8, slice, base, instances),
            mesh::SliceKind::Index16(ref buf, base) =>
                self.draw_indexed(buf, IntSize::U16, slice, base, instances),
            mesh::SliceKind::Index32(ref buf, base) =>
                self.draw_indexed(buf, IntSize::U32, slice, base, instances),
        }
    }

    fn clear_all<T>(&mut self,
                 color: Option<(&handle::RenderTargetView<R, T>, ColorValue)>,
                 depth: Option<(&handle::DepthStencilView<R, T>, Depth)>,
                 stencil: Option<(&handle::DepthStencilView<R, T>, Stencil)>)
    {
        use draw_state::target::{COLOR, DEPTH, STENCIL};
        use gfx_core::factory::Phantom;
        use gfx_core::pso::PixelTargetSet;

        let mut pts = PixelTargetSet::new();
        let mut mask = Mask::empty();
        let mut cdata = ClearData {
            color: [0.0; 4],
            depth: 0.0,
            stencil: 0,
        };
        if let Some((view, c)) = color {
            pts.colors[0] = Some(self.handles.ref_rtv(view.raw()).clone());
            mask = mask | COLOR;
            cdata.color = c;
        }
        if let Some((view, d)) = depth {
            pts.depth = Some(self.handles.ref_dsv(view.raw()).clone());
            mask = mask | DEPTH;
            cdata.depth = d;
        }
        if let Some((view, s)) = stencil {
            pts.stencil = Some(self.handles.ref_dsv(view.raw()).clone());
            mask = mask | STENCIL;
            cdata.stencil = s;
        }
        self.command_buffer.bind_pixel_targets(pts);
        self.command_buffer.call_clear(cdata, mask);
    }

    /// Clear a target view with a specified value.
    pub fn clear_target<T: format::RenderFormat>(&mut self,
                        view: &handle::RenderTargetView<R, T>,
                        value: ColorValue) { //TODO: value: T
        self.clear_all(Some((view, value)), None, None)
    }
    /// Clear a depth view with a specified value.
    pub fn clear_depth<T: format::DepthFormat>(&mut self,
                       view: &handle::DepthStencilView<R, T>, depth: Depth) {
        self.clear_all(None, Some((view, depth)), None)
    }

    /// Clear a stencil view with a specified value.
    pub fn clear_stencil<T: format::StencilFormat>(&mut self,
                         view: &handle::DepthStencilView<R, T>, stencil: Stencil) {
        self.clear_all(None, None, Some((view, stencil)))
    }

    /// Draw a mesh slice using a typed pipeline state object (PSO).
    pub fn draw_pipeline<D: pso::PipelineData<R>>(&mut self, slice: &mesh::Slice<R>,
                         pipeline: &pso::PipelineState<R, D::Meta>, user_data: &D)
    {
        let (pso, _) = self.handles.ref_pso(pipeline.get_handle());
        self.command_buffer.bind_pipeline_state(pso.clone());
        let raw_data = pipeline.prepare_data(user_data, &mut self.handles);
        self.command_buffer.bind_vertex_buffers(raw_data.vertex_buffers);
        self.command_buffer.bind_constant_buffers(raw_data.constant_buffers);
        for &(location, value) in &raw_data.global_constants {
            self.command_buffer.bind_global_constant(location, value);
        }
        self.command_buffer.bind_resource_views(raw_data.resource_views);
        self.command_buffer.bind_unordered_views(raw_data.unordered_views);
        self.command_buffer.bind_samplers(raw_data.samplers);
        self.command_buffer.bind_pixel_targets(raw_data.pixel_targets);
        self.draw_slice(slice, slice.instances);
    }
}
