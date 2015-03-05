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
use super::{shade, tex, Resources, BufferInfo};

/// Type-safe buffer handle
#[derive(Copy, Clone, PartialEq, Debug)]
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

    /// Get the underlying name for this Buffer
    pub fn get_name(&self) -> <R as Resources>::Buffer {
        self.raw.get_name()
    }

    /// Get the underlying raw Handle
    pub fn raw(&self) -> RawBuffer<R> {
        self.raw
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
#[derive(PartialEq, Debug)]
pub struct RawBuffer<R: Resources>(
    <R as Resources>::Buffer,
    BufferInfo
);

impl<R: Resources> Copy for RawBuffer<R> {}

impl<R: Resources> Clone for RawBuffer<R> {
    fn clone(&self) -> RawBuffer<R> {
        RawBuffer(self.0, self.1.clone())
    }
}

impl<R: Resources> RawBuffer<R> {
    /// Creates a new raw buffer handle (used by device)
    pub unsafe fn new(name: <R as Resources>::Buffer, info: BufferInfo)
        -> RawBuffer<R> {
        RawBuffer(name, info)
    }
    /// Get raw buffer name
    pub fn get_name(&self) -> <R as Resources>::Buffer { self.0 }
    /// Get raw buffer info
    pub fn get_info(&self) -> &BufferInfo { &self.1 }
}

/// Array Buffer Handle
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ArrayBuffer<R: Resources>(<R as Resources>::ArrayBuffer);

impl<R: Resources> ArrayBuffer<R> {
    /// Creates a new array buffer (used by device)
    pub unsafe fn new(name: <R as Resources>::ArrayBuffer)
        -> ArrayBuffer<R> {
        ArrayBuffer(name)
    }
    /// Get array buffer name
    pub fn get_name(&self) -> <R as Resources>::ArrayBuffer { self.0 }
}

/// Shader Handle
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Shader<R: Resources>(<R as Resources>::Shader, shade::Stage);

impl<R: Resources> Shader<R> {
    /// Creates a new shader (used by device)
    pub unsafe fn new(name: <R as Resources>::Shader, info: shade::Stage)
        -> Shader<R> {
        Shader(name, info)
    }
    /// Get shader name
    pub fn get_name(&self) -> <R as Resources>::Shader { self.0 }
    /// Get shader info
    pub fn get_info(&self) -> &shade::Stage { &self.1 }
}

/// Program Handle
#[derive(Clone, PartialEq, Debug)]
pub struct Program<R: Resources>(
    <R as Resources>::Program,
    shade::ProgramInfo,
);

impl<R: Resources> Program<R> {
    /// Creates a new program (used by device)
    pub unsafe fn new(name: <R as Resources>::Program, info: shade::ProgramInfo)
        -> Program<R> {
        Program(name, info)
    }
    /// Get program name
    pub fn get_name(&self) -> <R as Resources>::Program { self.0 }
    /// Get program info
    pub fn get_info(&self) -> &shade::ProgramInfo { &self.1 }
}

/// Frame Buffer Handle
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct FrameBuffer<R: Resources>(<R as Resources>::FrameBuffer);

impl<R: Resources> FrameBuffer<R> {
    /// Creates a new frame buffer (used by device)
    pub unsafe fn new(name: <R as Resources>::FrameBuffer)
        -> FrameBuffer<R> {
        FrameBuffer(name)
    }
    /// Get frame buffer name
    pub fn get_name(&self) -> <R as Resources>::FrameBuffer { self.0 }
}

/// Surface Handle
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Surface<R: Resources>(
    <R as Resources>::Surface,
    tex::SurfaceInfo,
);

impl<R: Resources> Surface<R> {
    /// Creates a new surface (used by device)
    pub unsafe fn new(name: <R as Resources>::Surface, info: tex::SurfaceInfo)
        -> Surface<R> {
        Surface(name, info)
    }
    /// Get surface name
    pub fn get_name(&self) -> <R as Resources>::Surface { self.0 }
    /// Get surface info
    pub fn get_info(&self) -> &tex::SurfaceInfo { &self.1 }
}

/// Texture Handle
#[derive(PartialEq, Debug)]
pub struct Texture<R: Resources>(
    <R as Resources>::Texture,
    tex::TextureInfo,
);

impl<R: Resources> Copy for Texture<R> {}

impl<R: Resources> Clone for Texture<R> {
    fn clone(&self) -> Texture<R> {
        Texture(self.0, self.1.clone())
    }
}

impl<R: Resources> Texture<R> {
    /// Creates a new texture (used by device)
    pub unsafe fn new(name: <R as Resources>::Texture, info: tex::TextureInfo)
        -> Texture<R> {
        Texture(name, info)
    }
    /// Get texture name
    pub fn get_name(&self) -> <R as Resources>::Texture { self.0 }
    /// Get texture info
    pub fn get_info(&self) -> &tex::TextureInfo { &self.1 }
}

/// Sampler Handle
#[derive(PartialEq, Debug)]
pub struct Sampler<R: Resources>(
    <R as Resources>::Sampler,
    tex::SamplerInfo,
);

impl<R: Resources> Copy for Sampler<R> {}

impl<R: Resources> Clone for Sampler<R> {
    fn clone(&self) -> Sampler<R> {
        Sampler(self.0, self.1.clone())
    }
}

impl<R: Resources> Sampler<R> {
    /// Creates a new sampler (used by device)
    pub unsafe fn new(name: <R as Resources>::Sampler, info: tex::SamplerInfo)
        -> Sampler<R> {
        Sampler(name, info)
    }
    /// Get sampler name
    pub fn get_name(&self) -> <R as Resources>::Sampler { self.0 }
    /// Get sampler info
    pub fn get_info(&self) -> &tex::SamplerInfo { &self.1 }
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
        super::Buffer {
            raw: unsafe { super::RawBuffer::new(
                (),
                BufferInfo {
                    usage: BufferUsage::Static,
                    size: mem::size_of::<T>() * len,
                },
            ) },
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
