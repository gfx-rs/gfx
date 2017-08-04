//! Resource components for a PSO.

use std::marker::PhantomData;
use core::{ResourceViewSlot, UnorderedViewSlot, SamplerSlot, Backend};
use core::{handle, pso, shade};
use core::memory::Typed;
use core::format::Format;
use super::{DataLink, DataBind, RawDataSet, AccessInfo};

/// Shader resource component (SRV). Typically is a view into some texture,
/// but can also be a buffer.
///
/// - init: `&str` = name of the resource
/// - data: `ShaderResourceView<T>`
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShaderResource<T>(
    RawShaderResource,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Raw (untyped) shader resource (SRV).
///
/// - init: `&str` = name of the resource. This may change in the future.
/// - data: `RawShaderResourceView`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawShaderResource(Option<(ResourceViewSlot, shade::Usage)>);

/// Unordered access component (UAV). A writable resource (texture/buffer)
/// with no defined access order across simultaneously executing shaders.
/// Supported on DX10 and higher.
///
/// - init: `&str` = name of the resource
/// - data: `UnorderedAccessView<T>`
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UnorderedAccess<T>(
    Option<(UnorderedViewSlot, shade::Usage)>,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Sampler component.
///
/// - init: `&str` = name of the sampler
/// - data: `Sampler`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Sampler(Option<(SamplerSlot, shade::Usage)>);

/// A convenience type for a texture paired with a sampler.
/// It only makes sense for DX9 class hardware, where every texture by default
/// is bundled with a sampler, hence they are represented by the same name.
/// In DX10 and higher samplers are totally separated from the textures.
///
/// - init: `&str` = name of the sampler/texture (assuming they match)
/// - data: (`ShaderResourceView<T>`, `Sampler`)
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextureSampler<T>(ShaderResource<T>, Sampler);

impl<'a, T> DataLink<'a> for ShaderResource<T> {
    type Init = &'a str;
    fn new() -> Self {
        ShaderResource(RawShaderResource(None), PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_resource_view(&mut self, var: &shade::TextureVar, init: &Self::Init)
                          -> Option<Result<pso::ResourceViewDesc, Format>> {
        self.0.link_resource_view(var, init)
    }
}

impl<B: Backend, T> DataBind<B> for ShaderResource<T> {
    type Data = handle::ShaderResourceView<B, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<B>,
               data: &Self::Data,
               man: &mut handle::Manager<B>,
               access: &mut AccessInfo<B>) {
        self.0.bind_to(out, data.raw(), man, access)
    }
}

impl<'a> DataLink<'a> for RawShaderResource {
    type Init = &'a str;
    fn new() -> Self {
        RawShaderResource(None)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_resource_view(&mut self, var: &shade::TextureVar, init: &Self::Init)
                          -> Option<Result<pso::ResourceViewDesc, Format>> {
        if *init == var.name {
            self.0 = Some((var.slot, var.usage));
            Some(Ok(var.usage)) //TODO: check format
        }else {
            None
        }
    }
}

impl<B: Backend> DataBind<B> for RawShaderResource {
    type Data = handle::RawShaderResourceView<B>;
    fn bind_to(&self,
               out: &mut RawDataSet<B>,
               data: &Self::Data,
               man: &mut handle::Manager<B>,
               _: &mut AccessInfo<B>) {
        // TODO: register buffer view source access
        if let Some((slot, usage)) = self.0 {
            let view = man.ref_srv(data).clone();
            out.resource_views.push(pso::ResourceViewParam(view, usage, slot));
        }
    }
}

impl<'a, T> DataLink<'a> for UnorderedAccess<T> {
    type Init = &'a str;
    fn new() -> Self {
        UnorderedAccess(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_unordered_view(&mut self, var: &shade::UnorderedVar, init: &Self::Init)
                           -> Option<Result<pso::UnorderedViewDesc, Format>> {
        if *init == var.name {
            self.0 = Some((var.slot, var.usage));
            Some(Ok(var.usage)) //TODO: check format
        }else {
            None
        }
    }
}

impl<B: Backend, T> DataBind<B> for UnorderedAccess<T> {
    type Data = handle::UnorderedAccessView<B, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<B>,
               data: &Self::Data,
               man: &mut handle::Manager<B>,
               _: &mut AccessInfo<B>) {
        // TODO: register buffer view source access
        if let Some((slot, usage)) = self.0 {
            let view =  man.ref_uav(data.raw()).clone();
            out.unordered_views.push(pso::UnorderedViewParam(view, usage, slot));
        }
    }
}

impl<'a> DataLink<'a> for Sampler {
    type Init = &'a str;
    fn new() -> Self {
        Sampler(None)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_sampler(&mut self, var: &shade::SamplerVar, init: &Self::Init)
                    -> Option<pso::SamplerDesc> {
        if *init == var.name {
            self.0 = Some((var.slot, var.usage));
            Some(var.usage)
        }else {
            None
        }
    }
}

impl<B: Backend> DataBind<B> for Sampler {
    type Data = handle::Sampler<B>;
    fn bind_to(&self,
               out: &mut RawDataSet<B>,
               data: &Self::Data,
               man: &mut handle::Manager<B>,
               _: &mut AccessInfo<B>) {
        if let Some((slot, usage)) = self.0 {
            let sm = man.ref_sampler(data).clone();
            out.samplers.push(pso::SamplerParam(sm, usage, slot));
        }
    }
}

impl<'a, T> DataLink<'a> for TextureSampler<T> {
    type Init = &'a str;
    fn new() -> Self {
        TextureSampler(ShaderResource::new(), Sampler::new())
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_resource_view(&mut self, var: &shade::TextureVar, init: &Self::Init)
                          -> Option<Result<pso::ResourceViewDesc, Format>> {
        self.0.link_resource_view(var, init)
    }
    fn link_sampler(&mut self, var: &shade::SamplerVar, init: &Self::Init) -> Option<pso::SamplerDesc> {
        self.1.link_sampler(var, init)
    }
}

impl<B: Backend, T> DataBind<B> for TextureSampler<T> {
    type Data = (handle::ShaderResourceView<B, T>, handle::Sampler<B>);
    fn bind_to(&self,
               out: &mut RawDataSet<B>,
               data: &Self::Data,
               man: &mut handle::Manager<B>,
               access: &mut AccessInfo<B>) {
        self.0.bind_to(out, &data.0, man, access);
        self.1.bind_to(out, &data.1, man, access);
    }
}
