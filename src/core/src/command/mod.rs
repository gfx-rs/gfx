// Copyright 2017 The Gfx-rs Developers.
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

//!

use {texture, Backend, InstanceCount, VertexCount};

use std::marker::PhantomData;

mod access;
mod compute;
mod general;
mod graphics;
mod raw;
mod transfer;

pub use self::access::AccessInfo;
pub use self::compute::ComputeCommandBuffer;
pub use self::general::GeneralCommandBuffer;
pub use self::graphics::GraphicsCommandBuffer;
pub use self::raw::RawCommandBuffer;
pub use self::transfer::TransferCommandBuffer;

/// A universal clear color supporting integet formats
/// as well as the standard floating-point.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum ClearColor {
    /// Standard floating-point vec4 color
    Float([f32; 4]),
    /// Integer vector to clear ivec4 targets.
    Int([i32; 4]),
    /// Unsigned int vector to clear uvec4 targets.
    Uint([u32; 4]),
}

macro_rules! impl_clear {
    { $( $ty:ty = $sub:ident[$a:expr, $b:expr, $c:expr, $d:expr], )* } => {
        $(
            impl From<$ty> for ClearColor {
                fn from(v: $ty) -> ClearColor {
                    ClearColor::$sub([v[$a], v[$b], v[$c], v[$d]])
                }
            }
        )*
    }
}

impl_clear! {
    [f32; 4] = Float[0, 1, 2, 3],
    [f32; 3] = Float[0, 1, 2, 0],
    [f32; 2] = Float[0, 1, 0, 0],
    [i32; 4] = Int  [0, 1, 2, 3],
    [i32; 3] = Int  [0, 1, 2, 0],
    [i32; 2] = Int  [0, 1, 0, 0],
    [u32; 4] = Uint [0, 1, 2, 3],
    [u32; 3] = Uint [0, 1, 2, 0],
    [u32; 2] = Uint [0, 1, 0, 0],
}

impl From<f32> for ClearColor {
    fn from(v: f32) -> ClearColor {
        ClearColor::Float([v, 0.0, 0.0, 0.0])
    }
}
impl From<i32> for ClearColor {
    fn from(v: i32) -> ClearColor {
        ClearColor::Int([v, 0, 0, 0])
    }
}
impl From<u32> for ClearColor {
    fn from(v: u32) -> ClearColor {
        ClearColor::Uint([v, 0, 0, 0])
    }
}

///
pub struct Offset {
    ///
    pub x: i32,
    ///
    pub y: i32,
    ///
    pub z: i32,
}

///
pub struct Extent {
    ///
    pub width: u32,
    ///
    pub height: u32,
    ///
    pub depth: u32,
}

/// Region of two buffers for copying.
pub struct BufferCopy {
    /// Buffer region source offset.
    pub src: u64,
    /// Buffer region destionation offset.
    pub dst: u64,
    /// Region size.
    pub size: u64,
}

pub struct BufferImageCopy {
    ///
    pub buffer_offset: u64,
    ///
    pub buffer_row_pitch: u32,
    ///
    pub buffer_slice_pitch: u32,
    ///
    pub image_mip_level: texture::Level,
    ///
    pub image_base_layer: texture::Layer,
    ///
    pub image_layers: texture::Layer,
    ///
    pub image_offset: Offset,
}

/// Optional instance parameters: (instance count, buffer offset)
pub type InstanceParams = (InstanceCount, VertexCount);

/// Thread-safe finished command buffer for submission.
pub struct Submit<B: Backend, C>(B::SubmitInfo, PhantomData<C>);
unsafe impl<B: Backend, C> Send for Submit<B, C> { }

impl<B: Backend, C> Submit<B, C> {
    ///
    pub(self) fn new(info: B::SubmitInfo) -> Self {
        Submit(info, PhantomData)
    }

    // Unsafe because we could try to submit a command buffer multiple times.
    #[doc(hidden)]
    pub unsafe fn get_info(&self) -> &B::SubmitInfo {
        &self.0
    }

    ///
    pub fn into_info(self) -> B::SubmitInfo {
        self.0
    }
}
