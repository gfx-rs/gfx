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

/// Type-safe buffer handle
#[derive(Clone, PartialEq, Debug)]
pub struct Buffer<R: Resources, T> {
    raw: RawBuffer<R>,
    phantom_t: PhantomData<T>,
}

impl<R: Resources, T> Buffer<R, T> {
    /// Create a type-safe BufferHandle from a RawBufferHandle
    pub fn from_raw(handle: RawBuffer<R>) -> Buffer<R, T> {
        Buffer {
            raw: handle,
            phantom_t: PhantomData,
        }
    }

    /// Cast the type this Buffer references
    pub fn cast<U>(self) -> Buffer<R, U> {
        Buffer::from_raw(self.raw)
    }

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

/// Raw (untyped) Buffer Handle
#[derive(Clone, PartialEq, Debug)]
pub struct RawBuffer<R: Resources>(Arc<R::Buffer>, BufferInfo);

impl<R: Resources> RawBuffer<R> {
    /// Get raw buffer info
    pub fn get_info(&self) -> &BufferInfo { &self.1 }
}

/// Array Buffer Handle
#[derive(Clone, PartialEq, Debug)]
pub struct ArrayBuffer<R: Resources>(Arc<R::ArrayBuffer>);

/// Shader Handle
#[derive(Clone, PartialEq, Debug)]
pub struct Shader<R: Resources>(Arc<R::Shader>, shade::Stage);

impl<R: Resources> Shader<R> {
    /// Get shader info
    pub fn get_info(&self) -> &shade::Stage { &self.1 }
}

/// Program Handle
#[derive(Clone, PartialEq, Debug)]
pub struct Program<R: Resources>(Arc<R::Program>, shade::ProgramInfo);

impl<R: Resources> Program<R> {
    /// Get program info
    pub fn get_info(&self) -> &shade::ProgramInfo { &self.1 }
}

/// Frame Buffer Handle
#[derive(Clone, PartialEq, Debug)]
pub struct FrameBuffer<R: Resources>(Arc<R::FrameBuffer>);

/// Surface Handle
#[derive(Clone, PartialEq, Debug)]
pub struct Surface<R: Resources>(Arc<R::Surface>, tex::SurfaceInfo);

impl<R: Resources> Surface<R> {
    /// Get surface info
    pub fn get_info(&self) -> &tex::SurfaceInfo { &self.1 }
}

/// Texture Handle
#[derive(Clone, PartialEq, Debug)]
pub struct Texture<R: Resources>(Arc<R::Texture>, tex::TextureInfo);

impl<R: Resources> Texture<R> {
    /// Get texture info
    pub fn get_info(&self) -> &tex::TextureInfo { &self.1 }
}

/// Sampler Handle
#[derive(Clone, PartialEq, Debug)]
pub struct Sampler<R: Resources>(Arc<R::Sampler>, tex::SamplerInfo);

impl<R: Resources> Sampler<R> {
    /// Get sampler info
    pub fn get_info(&self) -> &tex::SamplerInfo { &self.1 }
}


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
    frame_buffers: Vec<Arc<R::FrameBuffer>>,
    surfaces:      Vec<Arc<R::Surface>>,
    textures:      Vec<Arc<R::Texture>>,
    samplers:      Vec<Arc<R::Sampler>>,
}

/// A service trait to be used by the device implementation
#[allow(missing_docs)]
pub trait Producer<R: Resources> {
    fn make_buffer(&mut self, R::Buffer, BufferInfo) -> RawBuffer<R>;
    fn make_array_buffer(&mut self, R::ArrayBuffer) -> ArrayBuffer<R>;
    fn make_shader(&mut self, R::Shader, shade::Stage) -> Shader<R>;
    fn make_program(&mut self, R::Program, shade::ProgramInfo) -> Program<R>;
    fn make_frame_buffer(&mut self, R::FrameBuffer) -> FrameBuffer<R>;
    fn make_surface(&mut self, R::Surface, tex::SurfaceInfo) -> Surface<R>;
    fn make_texture(&mut self, R::Texture, tex::TextureInfo) -> Texture<R>;
    fn make_sampler(&mut self, R::Sampler, tex::SamplerInfo) -> Sampler<R>;
}

impl<R: Resources> Producer<R> for Manager<R> {
    fn make_buffer(&mut self, name: R::Buffer, info: BufferInfo) -> RawBuffer<R> {
        let r = Arc::new(name);
        self.buffers.push(r.clone());
        RawBuffer(r, info)
    }

    fn make_array_buffer(&mut self, name: R::ArrayBuffer) -> ArrayBuffer<R> {
        let r = Arc::new(name);
        self.array_buffers.push(r.clone());
        ArrayBuffer(r)
    }

    fn make_shader(&mut self, name: R::Shader, info: shade::Stage) -> Shader<R> {
        let r = Arc::new(name);
        self.shaders.push(r.clone());
        Shader(r, info)
    }

    fn make_program(&mut self, name: R::Program, info: shade::ProgramInfo) -> Program<R> {
        let r = Arc::new(name);
        self.programs.push(r.clone());
        Program(r, info)
    }

    fn make_frame_buffer(&mut self, name: R::FrameBuffer) -> FrameBuffer<R> {
        let r = Arc::new(name);
        self.frame_buffers.push(r.clone());
        FrameBuffer(r)
    }

    fn make_surface(&mut self, name: R::Surface, info: tex::SurfaceInfo) -> Surface<R> {
        let r = Arc::new(name);
        self.surfaces.push(r.clone());
        Surface(r, info)
    }

    fn make_texture(&mut self, name: R::Texture, info: tex::TextureInfo) -> Texture<R> {
        let r = Arc::new(name);
        self.textures.push(r.clone());
        Texture(r, info)
    }

    fn make_sampler(&mut self, name: R::Sampler, info: tex::SamplerInfo) -> Sampler<R> {
        let r = Arc::new(name);
        self.samplers.push(r.clone());
        Sampler(r, info)
    }
}

impl<R: Resources> Manager<R> {
    /// Create a new reference storage
    pub fn new() -> Manager<R> {
        Manager {
            buffers: Vec::new(),
            array_buffers: Vec::new(),
            shaders: Vec::new(),
            programs: Vec::new(),
            frame_buffers: Vec::new(),
            surfaces: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
        }
    }
    /// Clear all references
    pub fn clear(&mut self) {
        self.buffers.clear();
    }
    /// Extend with references from another manager
    pub fn extend(&mut self, other: &Manager<R>) {
        self.buffers.extend(other.buffers.iter().map(|b| b.clone()));
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
}


#[cfg(test)]
mod test {
    use std::mem;
    use std::marker::PhantomData;
    use device::{BufferInfo, BufferUsage, Resources};

    #[derive(Clone, Debug, PartialEq)]
    enum TestResources {}
    impl Resources for TestResources {
        type Buffer = ();
        type ArrayBuffer = ();
        type Shader = ();
        type Program = ();
        type FrameBuffer = ();
        type Surface = ();
        type Texture = ();
        type Sampler = ();
    }

    fn mock_buffer<T>(len: usize) -> super::Buffer<TestResources, T> {
        use super::Producer;
        let mut handler = super::Manager::new();
        super::Buffer {
            raw: handler.make_buffer((), BufferInfo {
                    usage: BufferUsage::Static,
                    size: mem::size_of::<T>() * len,
            }),
            phantom_t: PhantomData,
        }
    }

    #[test]
    fn test_buffer_len() {
        assert_eq!(mock_buffer::<u8>(8).len(), 8);
        assert_eq!(mock_buffer::<u16>(8).len(), 8);
    }

    #[test]
    #[should_fail]
    fn test_buffer_zero_len() {
        let _ = mock_buffer::<()>(0).len();
    }
}
