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

use std::error::Error;
use std::fmt;

pub use target::{Layer, Level};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CreationError { }

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        "Could not create image on device."
    }
}

/// Dimension size
pub type Size = u16;
/// Number of MSAA samples
pub type NumSamples = u8;
/// Number of EQAA fragments
pub type NumFragments = u8;

/// Describes the configuration of samples inside each texel.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum AaMode {
    /// No additional sample information
    Single,
    /// MultiSampled Anti-Aliasing (MSAA)
    Multi(NumSamples),
    /// Coverage Sampling Anti-Aliasing (CSAA/EQAA)
    Coverage(NumSamples, NumFragments),
}

impl From<NumSamples> for AaMode {
    fn from(ns: NumSamples) -> AaMode {
        if ns > 1 {
            AaMode::Multi(ns)
        } else {
            AaMode::Single
        }
    }
}

impl AaMode {
    /// Return the number of actual data fragments stored per texel.
    pub fn get_num_fragments(&self) -> NumFragments {
        match *self {
            AaMode::Single => 1,
            AaMode::Multi(n) => n,
            AaMode::Coverage(_, nf) => nf,
        }
    }
    /// Return true if the surface has to be resolved before sampling.
    pub fn needs_resolve(&self) -> bool {
        self.get_num_fragments() > 1
    }
}

/// Specifies the kind of a image storage to be allocated.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum Kind {
    /// A single row of texels.
    D1(Size),
    /// An array of rows of texels. Equivalent to Texture2D except that texels
    /// in a different row are not sampled.
    D1Array(Size, Layer),
    /// A traditional 2D image, with rows arranged contiguously.
    D2(Size, Size, AaMode),
    /// An array of 2D images. Equivalent to 3d image except that texels in
    /// a different depth level are not sampled.
    D2Array(Size, Size, Layer, AaMode),
    /// A volume image, with each 2D layer arranged contiguously.
    D3(Size, Size, Size),
    /// A set of 6 2D images, one for each face of a cube.
    Cube(Size),
    /// An array of Cube images.
    CubeArray(Size, Layer),
}

bitflags!(
    /// Image usage flags
    pub flags Usage: u8 {
        const TRANSFER_SRC    = 0x1,
        const TRANSFER_DST    = 0x2,
        const COLOR_ATTACHMENT  = 0x4,
        const DEPTH_STENCIL_ATTACHMENT = 0x8,
        const SAMPLED = 0x10,
        // TODO
    }
);
