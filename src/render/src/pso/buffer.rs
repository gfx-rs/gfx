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

//! Buffer components for a PSO.

use std::marker::PhantomData;
use core::{ConstantBufferSlot, Resources, MAX_VERTEX_ATTRIBUTES};
use core::{handle, pso, shade};
use core::memory::Typed;
use core::format::Format;
use shade::{ToUniform, Usage};
use super::{DataLink, DataBind, ElementError, RawDataSet, AccessInfo};

pub use core::pso::{BufferIndex, Element, ElemOffset, ElemStride, InstanceRate};

/// A trait to be implemented by any struct having the layout described
/// in the graphics API, like a vertex buffer.
pub trait Structure<F> {
    /// Get the layout of an element by name.
    fn query(&str) -> Option<Element<F>>;
}

type AttributeSlotSet = usize;

/// Service struct to simplify the implementations of `VertexBuffer` and `InstanceBuffer`.
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VertexBufferCommon<T, I=InstanceRate>(
    RawVertexBuffer,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<(T, I)>
);

/// Helper trait for `VertexBufferCommon` to support variable instance rate.
pub trait ToInstanceRate {
    /// The associated init type for PSO component.
    type Init;
    /// Get an actual instance rate value from the init.
    fn get_rate(init: &Self::Init) -> InstanceRate;
}

/// Helper phantom type for per-vertex attributes.
pub enum NonInstanced {}
/// Helper phantom type for per-instance attributes.
pub enum Instanced {}

impl ToInstanceRate for InstanceRate {
    type Init = InstanceRate;
    fn get_rate(init: &Self::Init) -> InstanceRate { *init }
}
impl ToInstanceRate for Instanced {
    type Init = ();
    fn get_rate(_: &Self::Init) -> InstanceRate { 1 }
}
impl ToInstanceRate for NonInstanced {
    type Init = ();
    fn get_rate(_: &Self::Init) -> InstanceRate { 0 }
}

/// Vertex buffer component. Advanced per vertex.
///
/// - init: `()`
/// - data: `Buffer<T>`
pub type VertexBuffer<T> = VertexBufferCommon<T, NonInstanced>;

/// Instance buffer component. Same as the vertex buffer but advances per instance.
pub type InstanceBuffer<T> = VertexBufferCommon<T, Instanced>;

/// Raw vertex/instance buffer component. Can be used when the formats of vertex attributes
/// are not known at compile time.
///
/// - init: `(&[&str, element], stride, inst_rate)`
/// - data: `RawBuffer`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawVertexBuffer(Option<BufferIndex>, AttributeSlotSet);

/// Constant buffer component.
///
/// - init: `&str` = name of the buffer
/// - data: `Buffer<T>`
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConstantBuffer<T: Structure<shade::ConstFormat>>(
    RawConstantBuffer,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Raw constant buffer component.
///
/// - init: `&str` = name of the buffer
/// - data: `RawBuffer`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawConstantBuffer(Option<(Usage, ConstantBufferSlot)>);

/// Global (uniform) constant component. Describes a free-standing value passed
/// into the shader, which is not enclosed into any constant buffer.
///
/// - init: `&str` = name of the constant
/// - data: `T` = value
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Global<T: ToUniform>(
    RawGlobal,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Raw global (uniform) constant component. Describes a free-standing value
/// passed into the shader, which is not enclosed in any constant buffer.
///
/// - init: `&str` = name of the constant
/// - data: `UniformValue` = value
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawGlobal(Option<shade::Location>);

fn match_attribute(attr: &shade::AttributeVar, fmt: Format) -> bool {
    use core::shade::{BaseType, ContainerType};
    use core::format::ChannelType;

    // TODO: Fill in `_` with compatible `core::format::SurfaceType` variants.

    match (attr.base_type, fmt.1) {
        (BaseType::I32, ChannelType::Int) |
        (BaseType::F32, ChannelType::Float) |
        (BaseType::F32, ChannelType::Inorm) |
        (BaseType::F32, ChannelType::Srgb) |
        (BaseType::F32, ChannelType::Unorm) |
        (BaseType::U32, ChannelType::Uint) => match (attr.container, fmt.0) {
                (ContainerType::Single, _) |
                (ContainerType::Vector(2), _) |
                (ContainerType::Vector(3), _) |
                (ContainerType::Vector(4), _) => true,
                _ => false,
        },
        (BaseType::F64, ChannelType::Float) |
        (BaseType::F64, ChannelType::Inorm) |
        (BaseType::F64, ChannelType::Srgb) |
        (BaseType::F64, ChannelType::Unorm) => match (attr.container, fmt.0) {
                (ContainerType::Single, _) |
                (ContainerType::Vector(2), _) |
                (ContainerType::Vector(3), _) |
                (ContainerType::Vector(4), _) => true,
                _ => false,
        },
        _ => false,
    }
}

impl<'a,
    T: Structure<Format>,
    I: ToInstanceRate + 'a,
> DataLink<'a> for VertexBufferCommon<T, I> {
    type Init = I::Init;
    fn new() -> Self {
        VertexBufferCommon(DataLink::new(), PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_vertex_buffer(&mut self, index: BufferIndex, init: &Self::Init)
                          -> Option<pso::VertexBufferDesc> {
        use std::mem;
        (self.0).0 = Some(index);
        let rate = I::get_rate(init);
        Some(pso::VertexBufferDesc {
            stride: mem::size_of::<T>() as ElemStride,
            rate: rate as InstanceRate,
        })
    }
    fn link_input(&mut self, at: &shade::AttributeVar, _: &Self::Init) ->
                  Option<Result<pso::AttributeDesc, Format>> {
        T::query(&at.name).map(|el| {
            self.0.link(at, el)
        })
    }
}

impl<R: Resources, T, I> DataBind<R> for VertexBufferCommon<T, I> {
    type Data = handle::Buffer<R, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               access: &mut AccessInfo<R>) {
        self.0.bind_to(out, data.raw(), man, access)
    }
}

impl RawVertexBuffer {
    fn link(&mut self, at: &shade::AttributeVar, el: Element<Format>)
            -> Result<pso::AttributeDesc, Format> {
        self.1 |= 1 << (at.slot as AttributeSlotSet);
        if match_attribute(at, el.format) {
            Ok((self.0.unwrap(), el))
        } else {
            Err(el.format)
        }
    }
}

impl<'a> DataLink<'a> for RawVertexBuffer {
    type Init = (&'a [(&'a str, Element<Format>)], ElemStride, InstanceRate);
    fn new() -> Self {
        RawVertexBuffer(None, 0)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_vertex_buffer(&mut self, index: BufferIndex, init: &Self::Init)
                          -> Option<pso::VertexBufferDesc> {
        self.0 = Some(index);
        Some(pso::VertexBufferDesc {
            stride: init.1,
            rate: init.2,
        })
    }
    fn link_input(&mut self, at: &shade::AttributeVar, init: &Self::Init) ->
                  Option<Result<pso::AttributeDesc, Format>> {
        init.0.iter().find(|x| x.0 == &at.name)
            .map(|x| self.link(at, x.1))
    }
}

impl<R: Resources> DataBind<R> for RawVertexBuffer {
    type Data = handle::RawBuffer<R>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               access: &mut AccessInfo<R>) {
        let value = Some((man.ref_buffer(data).clone(), 0));
        for i in 0 .. MAX_VERTEX_ATTRIBUTES {
            if (self.1 & (1<<i)) != 0 {
                out.vertex_buffers.0[i] = value;
            }
        }
        if self.1 != 0 { access.buffer_read(data); }
    }
}


impl<'a, T: Structure<shade::ConstFormat>>
DataLink<'a> for ConstantBuffer<T> {
    type Init = &'a str;
    fn new() -> Self {
        ConstantBuffer(RawConstantBuffer::new(), PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_constant_buffer<'b>(&mut self, cb: &'b shade::ConstantBufferVar, init: &Self::Init)
                            -> Option<Result<pso::ConstantBufferDesc, ElementError<&'b str>>> {
        let raw_out = self.0.link_constant_buffer(cb, init);
        if raw_out.is_some() {
            for el in cb.elements.iter() {
                let err = match T::query(&el.name) {
                    Some(e) if e.offset != el.location as pso::ElemOffset =>
                        ElementError::Offset {
                            name: el.name.as_str(),
                            shader_offset: el.location as pso::ElemOffset,
                            code_offset: e.offset,
                        },
                    None => ElementError::NotFound(el.name.as_str()),
                    Some(_) => continue, //TODO: check format
                };
                self.0 = RawConstantBuffer::new();
                return Some(Err(err));
            }
        }
        raw_out
    }
}

impl<R: Resources, T: Structure<shade::ConstFormat>>
DataBind<R> for ConstantBuffer<T> {
    type Data = handle::Buffer<R, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               access: &mut AccessInfo<R>) {
        self.0.bind_to(out, data.raw(), man, access)
    }
}

impl<'a> DataLink<'a> for RawConstantBuffer {
    type Init = &'a str;
    fn new() -> Self {
        RawConstantBuffer(None)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_constant_buffer<'b>(&mut self, cb: &'b shade::ConstantBufferVar, init: &Self::Init)
                            -> Option<Result<pso::ConstantBufferDesc, ElementError<&'b str>>> {
        if cb.name.as_str() == *init {
            self.0 = Some((cb.usage, cb.slot));
            Some(Ok(cb.usage))
        } else {
            None
        }
    }
}

impl<R: Resources> DataBind<R> for RawConstantBuffer {
    type Data = handle::RawBuffer<R>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               access: &mut AccessInfo<R>) {
        if let Some((usage, slot)) = self.0 {
            let buf = man.ref_buffer(data).clone();
            out.constant_buffers.push(pso::ConstantBufferParam(buf, usage, slot));
            access.buffer_read(data)
        }
    }
}

impl<'a, T: ToUniform + Default> DataLink<'a> for Global<T> {
    type Init = &'a str;
    fn new() -> Self {
        Global(RawGlobal::new(), PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_global_constant(&mut self, var: &shade::ConstVar, init: &Self::Init) ->
                            Option<Result<(), shade::CompatibilityError>> {
        let kind = ToUniform::convert(T::default());
        self.0.link_global_constant(var, init).and(Some(var.is_compatible(&kind)))
    }
}

impl<R: Resources, T: ToUniform> DataBind<R> for Global<T> {
    type Data = T;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               access: &mut AccessInfo<R>) {
        let val = data.convert();
        self.0.bind_to(out, &val, man, access);
    }
}

impl<'a> DataLink<'a> for RawGlobal {
    type Init = &'a str;
    fn new() -> Self {
        RawGlobal(None)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_global_constant(&mut self, var: &shade::ConstVar, init: &Self::Init) ->
                            Option<Result<(), shade::CompatibilityError>> {
        if var.name.as_str() == *init {
            self.0 = Some(var.location);
            Some(Ok(()))
        } else {
            None
        }
    }
}

impl<R: Resources> DataBind<R> for RawGlobal {
    type Data = shade::UniformValue;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               _: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        if let Some(loc) = self.0 {
            out.global_constants.push((loc, *data));
        }
    }
}
