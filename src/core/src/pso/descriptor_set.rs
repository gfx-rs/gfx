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

//! Descriptor sets and layouts.

use shade;

///
// TODO: Grasping and remembering the differences between these
//       types is a tough task. We might be able to come up with better names?
//       Or even use tuples to describe functionality instead of coming up with fancy names.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum DescriptorType {
    /// Controls filtering parameters for sampling from images.
    Sampler,
    /// Allows sampling (filtered loading) from associated image memory.
    /// Usually combined with a `Sampler`.
    SampledImage,
    /// Allows atomic operations, (non-filtered) loads and stores on image memory.
    StorageImage,
    /// Read-only, formatted buffer.
    UniformTexelBuffer,
    /// Read-Write, formatted buffer.
    StorageTexelBuffer,
    /// Read-only, structured buffer.
    ConstantBuffer,
    /// Read-Write, structured buffer.
    StorageBuffer,
    /// Allows unfiltered loads of pixel local data in the fragement shader.
    InputAttachment,

    // TODO: Dynamic descriptors
}

/// Binding descriptiong of a descriptor set
///
/// A descriptor set consists of multiple binding points.
/// Each binding point contains one or multiple descriptors of a certain type.
/// The binding point is only valid for the pipelines stages specified.
///
/// The binding _must_ match with the corresponding shader interface.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct DescriptorSetLayoutBinding {
    /// Integer identifier of the binding.
    pub binding: usize,
    /// Type of the bound descriptors.
    pub ty: DescriptorType,
    /// Number of descriptors bound.
    pub count: usize,
    /// Valid shader stages.
    pub stage_flags: shade::StageFlags,

    // TODO: immutable samplers?
}

/// Pool of descriptors of a specific type.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct DescriptorPoolDesc {
    /// Type of the stored descriptors.
    pub ty: DescriptorType,
    /// Amount of space.
    pub count: usize,
}
