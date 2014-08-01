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
// when cargo is ready, re-enable the `cfg`s
/* #[cfg(glfw)] */ extern crate glfw;
/* #[cfg(glfw)] */ extern crate glfw_platform;
/* #[cfg(gl-init)] */ extern crate gl_init_platform;
extern crate render;

// public re-exports
pub use render::{BufferHandle, ShaderHandle, ProgramHandle, SurfaceHandle, TextureHandle, SamplerHandle};
pub use render::Renderer;
pub use render::mesh::{Attribute, Mesh, VertexFormat, Slice, VertexSlice, IndexSlice};
pub use render::state::{DrawState, BlendAdditive, BlendAlpha};
pub use render::shade::{ProgramShell, CustomShell, ParameterSink, ShaderParam, ToUniform, TextureParam,
    ParameterLinkError, ParameterError, ErrorInternal, ErrorUniform, ErrorBlock, ErrorTexture,
    FnUniform, FnBlock, FnTexture, VarUniform, VarBlock, VarTexture};
pub use render::target::{Frame, Plane, PlaneEmpty, PlaneSurface, PlaneTexture};
pub use device::{attrib, state, tex};
pub use device::target::{Color, ClearData, Layer, Level};
pub use device::{Blob, Device, GlBackEnd, GlProvider, GraphicsContext, InitError, QueueSize};
pub use device::shade::{UniformValue, ValueI32, ValueF32, ValueI32Vec, ValueF32Vec, ValueF32Matrix};
pub use device::shade::{ShaderSource, StaticBytes};
/* #[cfg(glfw)] */ pub use GlfwWindowBuilder = glfw_platform::WindowBuilder;
/* #[cfg(glfw)] */ pub use GlfwContext = glfw_platform::Platform;
/* #[cfg(glfw)] */ pub use GlfwProvider = glfw_platform::Wrap;
/* #[cfg(gl-init)] */ pub use gl_init = gl_init_platform;

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

/// A builder for initializing gfx-rs using the GLFW library.
/* #[cfg(glfw)] */
pub type GlfwBuilder<'a, C> = Builder<
    SomeT<GlfwContext<C>>,
    SomeT<GlfwProvider<'a>>
>;

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

    /// Use GLFW for the context and provider.
    /* #[cfg(glfw)] */
    pub fn with_glfw<'a, C: glfw::Context>(self, glfw: &'a glfw::Glfw, context: C) -> GlfwBuilder<'a, C> {
        let (platform, provider) = glfw_platform::Platform::new(context, glfw);
        self.with_context(platform)
            .with_provider(provider)
    }

    /// Use GLFW for the context and provider, taking them from the supplied.
    /// `glfw::Window`.
    /* #[cfg(glfw)] */
    pub fn with_glfw_window<'a>(self, window: &'a mut glfw::Window) -> GlfwBuilder<'a, glfw::RenderContext> {
        let context = window.render_context();
        self.with_glfw(&window.glfw, context)
    }
}

/// Terminal builder methods. These can only be called once both the context
/// and provider have been supplied.
impl<C: GraphicsContext<GlBackEnd>, P: GlProvider> Builder<SomeT<C>, SomeT<P>> {
    /// Create a `Renderer` and `Device`.
    pub fn create(self) -> Result<(Renderer, Device<render::Token, GlBackEnd, C>), InitError> {
        let Builder { context: SomeT(context), provider: SomeT(provider), queue_size } = self;
        device::init(context, provider, queue_size).map(|(tx, rx, server, ack, should_finish)| {
            (Renderer::new(tx, rx, ack, should_finish), server)
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
