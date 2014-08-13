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

#![crate_name = "gfx"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(phase)]

//! An efficient, low-level, bindless graphics API for Rust. See [the
//! blog](http://gfx-rs.github.io/) for explanations and annotated examples.

#[phase(plugin, link)] extern crate log;
extern crate libc;

extern crate device;
extern crate render;

// public re-exports
pub use render::{ShaderHandle, ProgramHandle, SamplerHandle};
pub use render::Renderer;
pub use render::mesh::{Attribute, Mesh, VertexFormat, Slice, VertexSlice, IndexSlice};
pub use render::state::{DrawState, BlendAdditive, BlendAlpha};
pub use render::shade;
pub use render::target::{Frame, Plane, PlaneEmpty, PlaneSurface, PlaneTexture};
pub use device::{attrib, state, tex};
pub use device::{VertexCount, IndexCount};
pub use device::{Point, Line, LineStrip, TriangleList, TriangleStrip, TriangleFan};
pub use device::{Blob, Device, GlBackEnd, GlProvider, GraphicsContext, InitError, QueueSize};
pub use device::shade::{UniformValue, ValueI32, ValueF32, ValueI32Vec, ValueF32Vec, ValueF32Matrix};
pub use device::shade::{ShaderSource, StaticBytes};
pub use device::target::{Color, ClearData, Layer, Level};

/// The empty variant of a type-level option.
///
/// See also: `SomeT`.
pub struct NoneT;

/// The the 'some' variant of a type-level option.
///
/// See also: `NoneT`.
pub struct SomeT<T>(pub T);

/// A builder object used to initialise gfx-rs.
pub struct Builder<C, P> {
    pub context: C,
    pub provider: P,
    pub queue_size: QueueSize,
}

/// Create an empty builder object for initialising gfx-rs. The context and
/// provider *must* be supplied before initialisation can occur.
pub fn build() -> Builder<NoneT, NoneT> {
    Builder {
        context: NoneT,
        provider: NoneT,
        queue_size: 0,
    }
}

impl<C, P> Builder<C, P> {
    /// Supply a context to the builder.
    pub fn with_context<NewC: GraphicsContext<GlBackEnd>>(self, context: NewC)
            -> Builder<SomeT<NewC>, P> {
        let Builder { context: _, provider, queue_size } = self;
        Builder {
            context: SomeT(context),
            provider: provider,
            queue_size: queue_size,
        }
    }

    /// Supply a provider to the builder.
    pub fn with_provider<NewP: GlProvider>(self, provider: NewP)
            -> Builder<C, SomeT<NewP>> {
        let Builder { context, provider: _, queue_size } = self;
        Builder {
            context: context,
            provider: SomeT(provider),
            queue_size: queue_size,
        }
    }

    /// Supply a queue size to the builder.
    pub fn with_queue_size(mut self, queue_size: QueueSize) -> Builder<C, P> {
        self.queue_size = queue_size;
        self
    }
}

/// A type wrapper that hides `render::Token` and `Backend` from the user
pub type DeviceType<Context> = Device<render::Token, device::gl::GlBackEnd, Context>;

/// Terminal builder methods. These can only be called once both the context
/// and provider have been supplied.
impl<C: GraphicsContext<GlBackEnd>, P: GlProvider> Builder<SomeT<C>, SomeT<P>> {
    /// Create a `Renderer` and `Device`.
    pub fn create(self) -> Result<(Renderer, DeviceType<C>), InitError> {
        let Builder { context: SomeT(context), provider: SomeT(provider), queue_size } = self;
        device::init(context, provider, queue_size).map(|(tx, rx, server, ack)| {
            (Renderer::new(tx, rx, ack), server)
        })
    }

    /// Spawn the renderer in a new native task, and return the `Device`.
    pub fn spawn(self, f: proc(Renderer): 'static + Send) -> Result<Device<render::Token, GlBackEnd, C>, InitError> {
        match self.create() {
            Ok((renderer, device)) => {
                spawn(proc() f(renderer));
                Ok(device)
            },
            Err(e) => Err(e),
        }
    }
}
