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

use std::{cmp, hash, mem};
use std::marker::PhantomData;
use std::sync::Arc;
use {shade, tex, Resources};
use factory::{BufferInfo, Typed};


/// Raw (untyped) Buffer Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawBuffer<R: Resources>(Arc<R::Buffer>, BufferInfo);

impl<R: Resources> RawBuffer<R> {
    /// Get raw buffer info
    pub fn get_info(&self) -> &BufferInfo { &self.1 }
}

/// Type-safe buffer handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Buffer<R: Resources, T>(RawBuffer<R>, PhantomData<T>);

impl<R: Resources, T> Typed for Buffer<R, T> {
    type Raw = RawBuffer<R>;
    fn new(handle: RawBuffer<R>) -> Buffer<R, T> {
        Buffer(handle, PhantomData)
    }
    fn raw(&self) -> &RawBuffer<R> {
        &self.0
    }
}

impl<R: Resources, T> Buffer<R, T> {
    /// Get the associated information about the buffer
    pub fn get_info(&self) -> &BufferInfo {
        self.0.get_info()
    }

    /// Get the number of elements in the buffer.
    ///
    /// Fails if `T` is zero-sized.
    pub fn len(&self) -> usize {
        assert!(mem::size_of::<T>() != 0, "Cannot determine the length of zero-sized buffers.");
        self.get_info().size / mem::size_of::<T>()
    }
}

/// Shader Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Shader<R: Resources>(Arc<R::Shader>);

/// Program Handle
#[derive(Clone, Debug)]
pub struct Program<R: Resources>(Arc<R::Program>, shade::ProgramInfo);

// custom implementations due to the limitations of `ProgramInfo`
impl<R: Resources> Program<R> {
    /// Get program info
    pub fn get_info(&self) -> &shade::ProgramInfo { &self.1 }
}
impl<R: Resources> hash::Hash for Program<R> {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.0.hash(state);
    }
}
impl<R: Resources> cmp::PartialEq for Program<R> {
    fn eq(&self, other: &Program<R>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<R: Resources> cmp::Eq for Program<R> {}


/// Raw Pipeline State Handle
#[derive(Clone, Debug, PartialEq)]
pub struct RawPipelineState<R: Resources>(Arc<R::PipelineStateObject>, Arc<R::Program>);

/// Raw texture object
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawTexture<R: Resources>(Arc<R::Texture>, tex::Descriptor);

/// Typed texture object
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Texture<R: Resources, S>(RawTexture<R>, PhantomData<S>);

impl<R: Resources> RawTexture<R> {
    /// Get texture descriptor
    pub fn get_info(&self) -> &tex::Descriptor { &self.1 }
}

impl<R: Resources, S> Typed for Texture<R, S> {
    type Raw = RawTexture<R>;
    fn new(handle: RawTexture<R>) -> Texture<R, S> {
        Texture(handle, PhantomData)
    }
    fn raw(&self) -> &RawTexture<R> {
        &self.0
    }
}

impl<R: Resources, S> Texture<R, S> {
    /// Get texture descriptor
    pub fn get_info(&self) -> &tex::Descriptor { self.raw().get_info() }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ViewSource<R: Resources> {
    Buffer(Arc<R::Buffer>),
    Texture(Arc<R::Texture>),
}

/// Raw Shader Resource View Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawShaderResourceView<R: Resources>(Arc<R::ShaderResourceView>, ViewSource<R>);

/// Type-safe Shader Resource View Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShaderResourceView<R: Resources, T>(RawShaderResourceView<R>, PhantomData<T>);

impl<R: Resources, T> Typed for ShaderResourceView<R, T> {
    type Raw = RawShaderResourceView<R>;
    fn new(handle: RawShaderResourceView<R>) -> ShaderResourceView<R, T> {
        ShaderResourceView(handle, PhantomData)
    }
    fn raw(&self) -> &RawShaderResourceView<R> {
        &self.0
    }
}

/// Raw Unordered Access View Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawUnorderedAccessView<R: Resources>(Arc<R::UnorderedAccessView>, ViewSource<R>);

/// Type-safe Unordered Access View Handle
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UnorderedAccessView<R: Resources, T>(RawUnorderedAccessView<R>, PhantomData<T>);

impl<R: Resources, T> Typed for UnorderedAccessView<R, T> {
    type Raw = RawUnorderedAccessView<R>;
    fn new(handle: RawUnorderedAccessView<R>) -> UnorderedAccessView<R, T> {
        UnorderedAccessView(handle, PhantomData)
    }
    fn raw(&self) -> &RawUnorderedAccessView<R> {
        &self.0
    }
}

/// Raw RTV
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawRenderTargetView<R: Resources>(Arc<R::RenderTargetView>, Arc<R::Texture>, tex::Dimensions);

impl<R: Resources> RawRenderTargetView<R> {
    /// Get target dimensions
    pub fn get_dimensions(&self) -> tex::Dimensions {
        self.2
    }
}

/// Raw DSV
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawDepthStencilView<R: Resources>(Arc<R::DepthStencilView>, Arc<R::Texture>, tex::Dimensions);

impl<R: Resources> RawDepthStencilView<R> {
    /// Get target dimensions
    pub fn get_dimensions(&self) -> tex::Dimensions {
        self.2
    }
}

/// Typed RTV
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RenderTargetView<R: Resources, T>(RawRenderTargetView<R>, PhantomData<T>);

impl<R: Resources, T> RenderTargetView<R, T> {
    /// Get target dimensions
    pub fn get_dimensions(&self) -> tex::Dimensions {
        self.raw().get_dimensions()
    }
}

impl<R: Resources, T> Typed for RenderTargetView<R, T> {
    type Raw = RawRenderTargetView<R>;
    fn new(h: RawRenderTargetView<R>) -> RenderTargetView<R, T> {
        RenderTargetView(h, PhantomData)
    }
    fn raw(&self) -> &RawRenderTargetView<R> {
        &self.0
    }
}

/// Typed DSV
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DepthStencilView<R: Resources, T>(RawDepthStencilView<R>, PhantomData<T>);

impl<R: Resources, T> DepthStencilView<R, T> {
    /// Get target dimensions
    pub fn get_dimensions(&self) -> tex::Dimensions {
        self.raw().get_dimensions()
    }
}

impl<R: Resources, T> Typed for DepthStencilView<R, T> {
    type Raw = RawDepthStencilView<R>;
    fn new(h: RawDepthStencilView<R>) -> DepthStencilView<R, T> {
        DepthStencilView(h, PhantomData)
    }
    fn raw(&self) -> &RawDepthStencilView<R> {
        &self.0
    }
}

/// Sampler Handle
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sampler<R: Resources>(Arc<R::Sampler>, tex::SamplerInfo);

impl<R: Resources> Sampler<R> {
    /// Get sampler info
    pub fn get_info(&self) -> &tex::SamplerInfo { &self.1 }
}

/// Fence Handle
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fence<R: Resources>(Arc<R::Fence>);

/// Stores reference-counted resources used in a command buffer.
/// Seals actual resource names behind the interface, automatically
/// referencing them both by the Factory on resource creation
/// and the Renderer during CommandBuffer population.
#[allow(missing_docs)]
pub struct Manager<R: Resources> {
    buffers:       Vec<Arc<R::Buffer>>,
    shaders:       Vec<Arc<R::Shader>>,
    programs:      Vec<Arc<R::Program>>,
    psos:          Vec<Arc<R::PipelineStateObject>>,
    textures:      Vec<Arc<R::Texture>>,
    srvs:          Vec<Arc<R::ShaderResourceView>>,
    uavs:          Vec<Arc<R::UnorderedAccessView>>,
    rtvs:          Vec<Arc<R::RenderTargetView>>,
    dsvs:          Vec<Arc<R::DepthStencilView>>,
    samplers:      Vec<Arc<R::Sampler>>,
    fences:        Vec<Arc<R::Fence>>,
}

/// A service trait to be used by the device implementation
#[allow(missing_docs)]
pub trait Producer<R: Resources> {
    fn make_buffer(&mut self, R::Buffer, BufferInfo) -> RawBuffer<R>;
    fn make_shader(&mut self, R::Shader) -> Shader<R>;
    fn make_program(&mut self, R::Program, shade::ProgramInfo) -> Program<R>;
    fn make_pso(&mut self, R::PipelineStateObject, &Program<R>) -> RawPipelineState<R>;
    fn make_texture(&mut self, R::Texture, tex::Descriptor) -> RawTexture<R>;
    fn make_buffer_srv(&mut self, R::ShaderResourceView, &RawBuffer<R>) -> RawShaderResourceView<R>;
    fn make_texture_srv(&mut self, R::ShaderResourceView, &RawTexture<R>) -> RawShaderResourceView<R>;
    fn make_buffer_uav(&mut self, R::UnorderedAccessView, &RawBuffer<R>) -> RawUnorderedAccessView<R>;
    fn make_texture_uav(&mut self, R::UnorderedAccessView, &RawTexture<R>) -> RawUnorderedAccessView<R>;
    fn make_rtv(&mut self, R::RenderTargetView, &RawTexture<R>, tex::Dimensions) -> RawRenderTargetView<R>;
    fn make_dsv(&mut self, R::DepthStencilView, &RawTexture<R>, tex::Dimensions) -> RawDepthStencilView<R>;
    fn make_sampler(&mut self, R::Sampler, tex::SamplerInfo) -> Sampler<R>;
    fn make_fence(&mut self, name: R::Fence) -> Fence<R>;

    /// Walk through all the handles, keep ones that are reference elsewhere
    /// and call the provided delete function (resource-specific) for others
    fn clean_with<T,
        A: Fn(&mut T, &R::Buffer),
        B: Fn(&mut T, &R::Shader),
        C: Fn(&mut T, &R::Program),
        D: Fn(&mut T, &R::PipelineStateObject),
        E: Fn(&mut T, &R::Texture),
        F: Fn(&mut T, &R::ShaderResourceView),
        G: Fn(&mut T, &R::UnorderedAccessView),
        H: Fn(&mut T, &R::RenderTargetView),
        I: Fn(&mut T, &R::DepthStencilView),
        J: Fn(&mut T, &R::Sampler),
        K: Fn(&mut T, &R::Fence),
    >(&mut self, &mut T, A, B, C, D, E, F, G, H, I, J, K);
}

impl<R: Resources> Producer<R> for Manager<R> {
    fn make_buffer(&mut self, res: R::Buffer, info: BufferInfo) -> RawBuffer<R> {
        let r = Arc::new(res);
        self.buffers.push(r.clone());
        RawBuffer(r, info)
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

    fn make_pso(&mut self, res: R::PipelineStateObject, program: &Program<R>) -> RawPipelineState<R> {
        let r = Arc::new(res);
        self.psos.push(r.clone());
        RawPipelineState(r, program.0.clone())
    }

    fn make_texture(&mut self, res: R::Texture, desc: tex::Descriptor) -> RawTexture<R> {
        let r = Arc::new(res);
        self.textures.push(r.clone());
        RawTexture(r, desc)
    }

    fn make_buffer_srv(&mut self, res: R::ShaderResourceView, buf: &RawBuffer<R>) -> RawShaderResourceView<R> {
        let r = Arc::new(res);
        self.srvs.push(r.clone());
        RawShaderResourceView(r, ViewSource::Buffer(buf.0.clone()))
    }

    fn make_texture_srv(&mut self, res: R::ShaderResourceView, tex: &RawTexture<R>) -> RawShaderResourceView<R> {
        let r = Arc::new(res);
        self.srvs.push(r.clone());
        RawShaderResourceView(r, ViewSource::Texture(tex.0.clone()))
    }

    fn make_buffer_uav(&mut self, res: R::UnorderedAccessView, buf: &RawBuffer<R>) -> RawUnorderedAccessView<R> {
        let r = Arc::new(res);
        self.uavs.push(r.clone());
        RawUnorderedAccessView(r, ViewSource::Buffer(buf.0.clone()))
    }

    fn make_texture_uav(&mut self, res: R::UnorderedAccessView, tex: &RawTexture<R>) -> RawUnorderedAccessView<R> {
        let r = Arc::new(res);
        self.uavs.push(r.clone());
        RawUnorderedAccessView(r, ViewSource::Texture(tex.0.clone()))
    }

    fn make_rtv(&mut self, res: R::RenderTargetView, tex: &RawTexture<R>, dim: tex::Dimensions) -> RawRenderTargetView<R> {
        let r = Arc::new(res);
        self.rtvs.push(r.clone());
        RawRenderTargetView(r, tex.0.clone(), dim)
    }

    fn make_dsv(&mut self, res: R::DepthStencilView, tex: &RawTexture<R>, dim: tex::Dimensions) -> RawDepthStencilView<R> {
        let r = Arc::new(res);
        self.dsvs.push(r.clone());
        RawDepthStencilView(r, tex.0.clone(), dim)
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
        B: Fn(&mut T, &R::Shader),
        C: Fn(&mut T, &R::Program),
        D: Fn(&mut T, &R::PipelineStateObject),
        E: Fn(&mut T, &R::Texture),
        F: Fn(&mut T, &R::ShaderResourceView),
        G: Fn(&mut T, &R::UnorderedAccessView),
        H: Fn(&mut T, &R::RenderTargetView),
        I: Fn(&mut T, &R::DepthStencilView),
        J: Fn(&mut T, &R::Sampler),
        K: Fn(&mut T, &R::Fence),
    >(&mut self, param: &mut T, fa: A, fb: B, fc: C, fd: D, fe: E, ff: F, fg: G, fh: H, fi: I, fj: J, fk: K) {
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
        clean_vec(param, &mut self.shaders,       fb);
        clean_vec(param, &mut self.programs,      fc);
        clean_vec(param, &mut self.psos,          fd);
        clean_vec(param, &mut self.textures,      fe);
        clean_vec(param, &mut self.srvs,          ff);
        clean_vec(param, &mut self.uavs,          fg);
        clean_vec(param, &mut self.rtvs,          fh);
        clean_vec(param, &mut self.dsvs,          fi);
        clean_vec(param, &mut self.samplers,      fj);
        clean_vec(param, &mut self.fences,        fk);
    }
}

impl<R: Resources> Manager<R> {
    /// Create a new handle manager
    pub fn new() -> Manager<R> {
        Manager {
            buffers: Vec::new(),
            shaders: Vec::new(),
            programs: Vec::new(),
            psos: Vec::new(),
            textures: Vec::new(),
            srvs: Vec::new(),
            uavs: Vec::new(),
            rtvs: Vec::new(),
            dsvs: Vec::new(),
            samplers: Vec::new(),
            fences: Vec::new()
        }
    }
    /// Clear all references
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.shaders.clear();
        self.programs.clear();
        self.psos.clear();
        self.textures.clear();
        self.srvs.clear();
        self.uavs.clear();
        self.rtvs.clear();
        self.dsvs.clear();
        self.samplers.clear();
    }
    /// Extend with all references of another handle manager
    pub fn extend(&mut self, other: &Manager<R>) {
        self.buffers  .extend(other.buffers  .iter().map(|h| h.clone()));
        self.shaders  .extend(other.shaders  .iter().map(|h| h.clone()));
        self.programs .extend(other.programs .iter().map(|h| h.clone()));
        self.psos     .extend(other.psos     .iter().map(|h| h.clone()));
        self.textures .extend(other.textures .iter().map(|h| h.clone()));
        self.srvs     .extend(other.srvs     .iter().map(|h| h.clone()));
        self.uavs     .extend(other.uavs     .iter().map(|h| h.clone()));
        self.rtvs     .extend(other.rtvs     .iter().map(|h| h.clone()));
        self.dsvs     .extend(other.dsvs     .iter().map(|h| h.clone()));
        self.samplers .extend(other.samplers .iter().map(|h| h.clone()));
    }
    /// Count the total number of referenced resources
    pub fn count(&self) -> usize {
        self.buffers.len() +
        self.shaders.len() +
        self.programs.len() +
        self.psos.len() +
        self.textures.len() +
        self.srvs.len() +
        self.uavs.len() +
        self.rtvs.len() +
        self.dsvs.len() +
        self.samplers.len()
    }
    /// Reference a buffer
    pub fn ref_buffer<'a>(&mut self, handle: &'a RawBuffer<R>) -> &'a R::Buffer {
        self.buffers.push(handle.0.clone());
        &handle.0
    }
    /// Reference a shader
    pub fn ref_shader<'a>(&mut self, handle: &'a Shader<R>) -> &'a R::Shader {
        self.shaders.push(handle.0.clone());
        &handle.0
    }
    /// Reference a program
    pub fn ref_program<'a>(&mut self, handle: &'a Program<R>) -> &'a R::Program {
        self.programs.push(handle.0.clone());
        &handle.0
    }
    /// Reference a pipeline state object
    pub fn ref_pso<'a>(&mut self, handle: &'a RawPipelineState<R>) -> (&'a R::PipelineStateObject, &'a R::Program) {
        self.psos.push(handle.0.clone());
        self.programs.push(handle.1.clone());
        (&handle.0, &handle.1)
    }
    /// Reference a texture
    pub fn ref_texture<'a>(&mut self, handle: &'a RawTexture<R>) -> &'a R::Texture {
        self.textures.push(handle.0.clone());
        &handle.0
    }
    /// Reference a shader resource view
    pub fn ref_srv<'a>(&mut self, handle: &'a RawShaderResourceView<R>) -> &'a R::ShaderResourceView {
        self.srvs.push(handle.0.clone());
        &handle.0
    }
    /// Reference an unordered access view
    pub fn ref_uav<'a>(&mut self, handle: &'a RawUnorderedAccessView<R>) -> &'a R::UnorderedAccessView {
        self.uavs.push(handle.0.clone());
        &handle.0
    }
    /// Reference an RTV
    pub fn ref_rtv<'a>(&mut self, handle: &'a RawRenderTargetView<R>) -> &'a R::RenderTargetView {
        self.rtvs.push(handle.0.clone());
        self.textures.push(handle.1.clone());
        &handle.0
    }
    /// Reference a DSV
    pub fn ref_dsv<'a>(&mut self, handle: &'a RawDepthStencilView<R>) -> &'a R::DepthStencilView {
        self.dsvs.push(handle.0.clone());
        self.textures.push(handle.1.clone());
        &handle.0
    }
    /// Reference a sampler
    pub fn ref_sampler<'a>(&mut self, handle: &'a Sampler<R>) -> &'a R::Sampler {
        self.samplers.push(handle.0.clone());
        &handle.0
    }
    /// Reference a fence
    pub fn ref_fence<'a>(&mut self, fence: &'a Fence<R>) -> &'a R::Fence {
        self.fences.push(fence.0.clone());
        &fence.0
    }
}
