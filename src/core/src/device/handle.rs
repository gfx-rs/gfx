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

#![deny(missing_docs, missing_copy_implementations)]

//! Device resource handles

use std::mem;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use super::{shade, tex, Resources, BufferInfo};


/// Raw (untyped) Buffer Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawBuffer<R: Resources>(Arc<R::Buffer>, BufferInfo);

impl<R: Resources> RawBuffer<R> {
    /// Get raw buffer info
    pub fn get_info(&self) -> &BufferInfo { &self.1 }
}

/// Type-safe buffer handle
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Buffer<R: Resources, T> {
    raw: RawBuffer<R>,
    phantom_t: PhantomData<T>,
}

impl<R: Resources, T> From<RawBuffer<R>> for Buffer<R, T> {
    fn from(handle: RawBuffer<R>) -> Buffer<R, T> {
        Buffer {
            raw: handle,
            phantom_t: PhantomData,
        }
    }
}

impl<R: Resources, T> Buffer<R, T> {
    /// Get the underlying raw Handle
    pub fn raw(&self) -> &RawBuffer<R> {
        &self.raw
    }

    /// Get the associated information about the buffer
    pub fn get_info(&self) -> &BufferInfo {
        self.raw.get_info()
    }

    /// Get the number of elements in the buffer.
    ///
    /// Fails if `T` is zero-sized.
    pub fn len(&self) -> usize {
        assert!(mem::size_of::<T>() != 0, "Cannot determine the length of zero-sized buffers.");
        self.get_info().size / mem::size_of::<T>()
    }
}

/// Array Buffer Handle
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ArrayBuffer<R: Resources>(Arc<R::ArrayBuffer>);

/// Shader Handle
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Shader<R: Resources>(Arc<R::Shader>);

/// Program Handle
#[derive(Clone, Debug, PartialEq)]
pub struct Program<R: Resources>(Arc<R::Program>, shade::ProgramInfo);

impl<R: Resources> Program<R> {
    /// Get program info
    pub fn get_info(&self) -> &shade::ProgramInfo { &self.1 }
}

/// Pipeline State Handle
#[derive(Clone, Debug, PartialEq)]
pub struct RawPipelineState<R: Resources>(Arc<R::PipelineStateObject>,);

/// Frame Buffer Handle
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FrameBuffer<R: Resources>(Arc<R::FrameBuffer>);

/// Surface Handle
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Surface<R: Resources>(Arc<R::Surface>, tex::SurfaceInfo);

impl<R: Resources> Surface<R> {
    /// Get surface info
    pub fn get_info(&self) -> &tex::SurfaceInfo { &self.1 }
}

/// Convenience target view prototype, applicable to both colors and depth/stencil
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProtoTargetView<R: Resources, X>(Arc<X>, Texture<R>);

impl<R: Resources, X> ProtoTargetView<R, X> {
    /// Get target texture
    pub fn get_texture(&self) -> &Texture<R> { &self.1 }
}

/// Raw RTV
pub type RawRenderTargetView<R: Resources> = ProtoTargetView<R, R::RenderTargetView>;
/// Raw DSV
pub type RawDepthStencilView<R: Resources> = ProtoTargetView<R, R::DepthStencilView>;

/// A target view template that is equally applicable to colors and depth/stencil
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetView<R: Resources, X, T>(ProtoTargetView<R, X>, PhantomData<T>);

impl<R: Resources, X, T> From<ProtoTargetView<R, X>> for TargetView<R, X, T> {
    fn from(h: ProtoTargetView<R, X>) -> TargetView<R, X, T> {
        TargetView(h, PhantomData)
    }
}

impl<R: Resources, X, T> TargetView<R, X, T> {
    /// Get the underlying raw Handle
    pub fn raw(&self) -> &ProtoTargetView<R, X> {
        &self.0
    }
}

/// Typed RTV
pub type RenderTargetView<R: Resources, T> = TargetView<R, R::RenderTargetView, T>;
/// Typed DSV
pub type DepthStencilView<R: Resources, T> = TargetView<R, R::DepthStencilView, T>; 

/// Texture Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Texture<R: Resources>(Arc<R::Texture>, tex::TextureInfo);

impl<R: Resources> Texture<R> {
    /// Get texture info
    pub fn get_info(&self) -> &tex::TextureInfo { &self.1 }
}

/// Sampler Handle
#[derive(Clone, Debug, PartialEq)]
pub struct Sampler<R: Resources>(Arc<R::Sampler>, tex::SamplerInfo);

impl<R: Resources> Sampler<R> {
    /// Get sampler info
    pub fn get_info(&self) -> &tex::SamplerInfo { &self.1 }
}

/// Fence Handle
#[derive(Clone, Debug, PartialEq)]
pub struct Fence<R: Resources>(Arc<R::Fence>);

/// Stores reference-counted resources used in a command buffer.
/// Seals actual resource names behind the interface, automatically
/// referencing them both by the Factory on resource creation
/// and the Renderer during CommandBuffer population.
#[allow(missing_docs)]
pub struct Manager<R: Resources> {
    buffers:       Vec<Arc<R::Buffer>>,
    array_buffers: Vec<Arc<R::ArrayBuffer>>,
    shaders:       Vec<Arc<R::Shader>>,
    programs:      Vec<Arc<R::Program>>,
    psos:          Vec<Arc<R::PipelineStateObject>>,
    frame_buffers: Vec<Arc<R::FrameBuffer>>,
    surfaces:      Vec<Arc<R::Surface>>,
    rtvs:          Vec<Arc<R::RenderTargetView>>,
    dsvs:          Vec<Arc<R::DepthStencilView>>,
    textures:      Vec<Arc<R::Texture>>,
    samplers:      Vec<Arc<R::Sampler>>,
    fences:        Vec<Arc<R::Fence>>,
}

/// A service trait to be used by the device implementation
#[allow(missing_docs)]
pub trait Producer<R: Resources> {
    fn make_buffer(&mut self, R::Buffer, BufferInfo) -> RawBuffer<R>;
    fn make_array_buffer(&mut self, R::ArrayBuffer) -> ArrayBuffer<R>;
    fn make_shader(&mut self, R::Shader) -> Shader<R>;
    fn make_program(&mut self, R::Program, shade::ProgramInfo) -> Program<R>;
    fn make_pso(&mut self, R::PipelineStateObject) -> RawPipelineState<R>;
    fn make_frame_buffer(&mut self, R::FrameBuffer) -> FrameBuffer<R>;
    fn make_surface(&mut self, R::Surface, tex::SurfaceInfo) -> Surface<R>;
    fn make_rtv(&mut self, R::RenderTargetView, Texture<R>) -> RawRenderTargetView<R>;
    fn make_dsv(&mut self, R::DepthStencilView, Texture<R>) -> RawDepthStencilView<R>;
    fn make_texture(&mut self, R::Texture, tex::TextureInfo) -> Texture<R>;
    fn make_sampler(&mut self, R::Sampler, tex::SamplerInfo) -> Sampler<R>;
    fn make_fence(&mut self, name: R::Fence) -> Fence<R>;

    /// Walk through all the handles, keep ones that are reference elsewhere
    /// and call the provided delete function (resource-specific) for others
    fn clean_with<T,
        A: Fn(&mut T, &R::Buffer),
        B: Fn(&mut T, &R::ArrayBuffer),
        C: Fn(&mut T, &R::Shader),
        D: Fn(&mut T, &R::Program),
        E: Fn(&mut T, &R::PipelineStateObject),
        F: Fn(&mut T, &R::FrameBuffer),
        G: Fn(&mut T, &R::Surface),
        H: Fn(&mut T, &R::RenderTargetView),
        I: Fn(&mut T, &R::DepthStencilView),
        J: Fn(&mut T, &R::Texture),
        K: Fn(&mut T, &R::Sampler),
        L: Fn(&mut T, &R::Fence),
    >(&mut self, &mut T, A, B, C, D, E, F, G, H, I, J, K, L);
}

impl<R: Resources> Producer<R> for Manager<R> {
    fn make_buffer(&mut self, res: R::Buffer, info: BufferInfo) -> RawBuffer<R> {
        let r = Arc::new(res);
        self.buffers.push(r.clone());
        RawBuffer(r, info)
    }

    fn make_array_buffer(&mut self, res: R::ArrayBuffer) -> ArrayBuffer<R> {
        let r = Arc::new(res);
        self.array_buffers.push(r.clone());
        ArrayBuffer(r)
    }

    fn make_shader(&mut self, res: R::Shader) -> Shader<R> {
        let r = Arc::new(res);
        self.shaders.push(r.clone());
        Shader(r)
    }

    fn make_program(&mut self, res: R::Program, info: shade::ProgramInfo) -> Program<R> {
        let r = Arc::new(res);
        self.programs.push(r.clone());
        Program(r, info)
    }

    fn make_pso(&mut self, res: R::PipelineStateObject) -> RawPipelineState<R> {
        let r = Arc::new(res);
        self.psos.push(r.clone());
        RawPipelineState(r)
    }

    fn make_frame_buffer(&mut self, res: R::FrameBuffer) -> FrameBuffer<R> {
        let r = Arc::new(res);
        self.frame_buffers.push(r.clone());
        FrameBuffer(r)
    }

    fn make_surface(&mut self, res: R::Surface, info: tex::SurfaceInfo) -> Surface<R> {
        let r = Arc::new(res);
        self.surfaces.push(r.clone());
        Surface(r, info)
    }

    fn make_rtv(&mut self, res: R::RenderTargetView, tex: Texture<R>) -> RawRenderTargetView<R> {
        let r = Arc::new(res);
        self.rtvs.push(r.clone());
        ProtoTargetView(r, tex)
    }

    fn make_dsv(&mut self, res: R::DepthStencilView, tex: Texture<R>) -> RawDepthStencilView<R> {
        let r = Arc::new(res);
        self.dsvs.push(r.clone());
        ProtoTargetView(r, tex)
    }

    fn make_texture(&mut self, res: R::Texture, info: tex::TextureInfo) -> Texture<R> {
        let r = Arc::new(res);
        self.textures.push(r.clone());
        Texture(r, info)
    }

    fn make_sampler(&mut self, res: R::Sampler, info: tex::SamplerInfo) -> Sampler<R> {
        let r = Arc::new(res);
        self.samplers.push(r.clone());
        Sampler(r, info)
    }

    fn make_fence(&mut self, res: R::Fence) -> Fence<R> {
        let r = Arc::new(res);
        self.fences.push(r.clone());
        Fence(r)
    }

    fn clean_with<T,
        A: Fn(&mut T, &R::Buffer),
        B: Fn(&mut T, &R::ArrayBuffer),
        C: Fn(&mut T, &R::Shader),
        D: Fn(&mut T, &R::Program),
        E: Fn(&mut T, &R::PipelineStateObject),
        F: Fn(&mut T, &R::FrameBuffer),
        G: Fn(&mut T, &R::Surface),
        H: Fn(&mut T, &R::RenderTargetView),
        I: Fn(&mut T, &R::DepthStencilView),
        J: Fn(&mut T, &R::Texture),
        K: Fn(&mut T, &R::Sampler),
        L: Fn(&mut T, &R::Fence),
    >(&mut self, param: &mut T, fa: A, fb: B, fc: C, fd: D, fe: E, ff: F, fg: G, fh: H, fi: I, fj: J, fk: K, fl: L) {
        fn clean_vec<X, Param, Fun>(param: &mut Param, vector: &mut Vec<Arc<X>>, fun: Fun)
            where X: Clone, Fun: Fn(&mut Param, &X)
        {
            let mut temp = Vec::new();
            // delete unique resources and make a list of their indices
            for (i, v) in vector.iter_mut().enumerate() {
                if let Some(x) = Arc::get_mut(v) {
                    fun(param, x);
                    temp.push(i);
                }
            }
            // update the resource vector by removing the elements
            // starting from the last one
            for t in temp.iter().rev() {
                vector.swap_remove(*t);
            }
        }
        clean_vec(param, &mut self.buffers,       fa);
        clean_vec(param, &mut self.array_buffers, fb);
        clean_vec(param, &mut self.shaders,       fc);
        clean_vec(param, &mut self.programs,      fd);
        clean_vec(param, &mut self.psos,          fe);
        clean_vec(param, &mut self.frame_buffers, ff);
        clean_vec(param, &mut self.surfaces,      fg);
        clean_vec(param, &mut self.rtvs,          fh);
        clean_vec(param, &mut self.dsvs,          fi);
        clean_vec(param, &mut self.textures,      fj);
        clean_vec(param, &mut self.samplers,      fk);
        clean_vec(param, &mut self.fences,        fl);
    }
}

impl<R: Resources> Manager<R> {
    /// Create a new handle manager
    pub fn new() -> Manager<R> {
        Manager {
            buffers: Vec::new(),
            array_buffers: Vec::new(),
            shaders: Vec::new(),
            programs: Vec::new(),
            psos: Vec::new(),
            frame_buffers: Vec::new(),
            surfaces: Vec::new(),
            rtvs: Vec::new(),
            dsvs: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
            fences: Vec::new()
        }
    }
    /// Clear all references
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.array_buffers.clear();
        self.shaders.clear();
        self.programs.clear();
        self.psos.clear();
        self.frame_buffers.clear();
        self.surfaces.clear();
        self.rtvs.clear();
        self.dsvs.clear();
        self.textures.clear();
        self.samplers.clear();
    }
    /// Extend with all references of another handle manager
    pub fn extend(&mut self, other: &Manager<R>) {
        self.buffers      .extend(other.buffers      .iter().map(|h| h.clone()));
        self.array_buffers.extend(other.array_buffers.iter().map(|h| h.clone()));
        self.shaders      .extend(other.shaders      .iter().map(|h| h.clone()));
        self.programs     .extend(other.programs     .iter().map(|h| h.clone()));
        self.psos         .extend(other.psos         .iter().map(|h| h.clone()));
        self.frame_buffers.extend(other.frame_buffers.iter().map(|h| h.clone()));
        self.surfaces     .extend(other.surfaces     .iter().map(|h| h.clone()));
        self.rtvs         .extend(other.rtvs         .iter().map(|h| h.clone()));
        self.dsvs         .extend(other.dsvs         .iter().map(|h| h.clone()));
        self.textures     .extend(other.textures     .iter().map(|h| h.clone()));
        self.samplers     .extend(other.samplers     .iter().map(|h| h.clone()));
    }
    /// Count the total number of referenced resources
    pub fn count(&self) -> usize {
        self.buffers.len() +
        self.array_buffers.len() +
        self.shaders.len() +
        self.programs.len() +
        self.psos.len() +
        self.frame_buffers.len() +
        self.surfaces.len() +
        self.rtvs.len() +
        self.dsvs.len() +
        self.textures.len() +
        self.samplers.len()
    }
    /// Reference a buffer
    pub fn ref_buffer(&mut self, handle: &RawBuffer<R>) -> R::Buffer {
        self.buffers.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference am array buffer
    pub fn ref_array_buffer(&mut self, handle: &ArrayBuffer<R>) -> R::ArrayBuffer {
        self.array_buffers.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a shader
    pub fn ref_shader(&mut self, handle: &Shader<R>) -> R::Shader {
        self.shaders.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a program
    pub fn ref_program(&mut self, handle: &Program<R>) -> R::Program {
        self.programs.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a ppipeline state object
    pub fn ref_pso(&mut self, handle: &RawPipelineState<R>) -> R::PipelineStateObject {
        self.psos.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a frame buffer
    pub fn ref_frame_buffer(&mut self, handle: &FrameBuffer<R>) -> R::FrameBuffer {
        self.frame_buffers.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a surface
    pub fn ref_surface(&mut self, handle: &Surface<R>) -> R::Surface {
        self.surfaces.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference an RTV
    pub fn ref_rtv(&mut self, handle: &RawRenderTargetView<R>) -> R::RenderTargetView {
        self.rtvs.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a DSV
    pub fn ref_dsv(&mut self, handle: &RawDepthStencilView<R>) -> R::DepthStencilView {
        self.dsvs.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a texture
    pub fn ref_texture(&mut self, handle: &Texture<R>) -> R::Texture {
        self.textures.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a sampler
    pub fn ref_sampler(&mut self, handle: &Sampler<R>) -> R::Sampler {
        self.samplers.push(handle.0.clone());
        *handle.0.deref()
    }
    /// Reference a fence
    pub fn ref_fence(&mut self, fence: &Fence<R>) -> R::Fence {
        self.fences.push(fence.0.clone());
        *fence.0.deref()
    }
}
