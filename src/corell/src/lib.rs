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

#[macro_use]
extern crate log;

use std::fmt::Debug;
use std::hash::Hash;
use std::any::Any;

pub mod command;
pub mod factory;
pub mod format;
pub mod memory;

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw number of instances
pub type InstanceCount = u32;
/// Number of vertices in a patch
pub type PatchSize = u8;

/// Describes what geometric primitives are created from vertex data.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum Primitive {
    /// Each vertex represents a single point.
    PointList,
    /// Each pair of vertices represent a single line segment. For example, with `[a, b, c, d,
    /// e]`, `a` and `b` form a line, `c` and `d` form a line, and `e` is discarded.
    LineList,
    /// Every two consecutive vertices represent a single line segment. Visually forms a "path" of
    /// lines, as they are all connected. For example, with `[a, b, c]`, `a` and `b` form a line
    /// line, and `b` and `c` form a line.
    LineStrip,
    /// Each triplet of vertices represent a single triangle. For example, with `[a, b, c, d, e]`,
    /// `a`, `b`, and `c` form a triangle, `d` and `e` are discarded.
    TriangleList,
    /// Every three consecutive vertices represent a single triangle. For example, with `[a, b, c,
    /// d]`, `a`, `b`, and `c` form a triangle, and `b`, `c`, and `d` form a triangle.
    TriangleStrip,
    /// Patch list,
    /// used with shaders capable of producing primitives on their own (tessellation)
    PatchList(PatchSize),
}

/// A type of each index value in the slice's index buffer
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(missing_docs)]
#[repr(u8)]
pub enum IndexType {
    U16,
    U32,
}

/// An `Instance` holds per-application state for a specific backend
pub trait Instance {
    type B: Backend;

    /// Instantiate a new `Instance`, this is our entry point for applications
    fn create() -> Self;

    // TODO: Use an iterator instead of Vec?
    /// Enumerate all available devices supporting this backend 
    fn enumerate_physical_devices(&self) -> Vec<<<Self as Instance>::B as Backend>::PhysicalDevice>;
}

// TODO: Name might be a bit misleading as we might also support
// software devices (e.g D3D12's WARP) and maybe multi-GPUs in the future (not part of Vulkan 1.0)
// D3D12's `Adapter` would be a possible alternative
pub trait PhysicalDevice {
    type B: Backend;

    // TODO: Let the users decide how many and which queues they want to create
    fn open(&self) -> (<<Self as PhysicalDevice>::B as Backend>::Device, Vec<<<Self as PhysicalDevice>::B as Backend>::CommandQueue>);

    fn get_info(&self) -> &PhysicalDeviceInfo;
}

#[derive(Clone, Debug)]
pub struct PhysicalDeviceInfo {
    /// Phyiscal device name
    pub name: String,
    /// Vendor PCI id of the physical device
    pub vendor: usize,
    /// PCI id of the physical device
    pub device: usize,
    /// The device is based on a software rasterizer
    pub software_rendering: bool,
}

pub trait Device {
    
}

pub trait CommandQueue {
    type B: Backend;

    /// Submits a `CommandBuffer` to the GPU queue for execution.
    fn submit(&mut self, cmd_buffer: &<<Self as CommandQueue>::B as Backend>::CommandBuffer);
}

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface {

}

/// The `SwapChain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait SwapChain {
    type B: Backend;

    fn present(&mut self);
}

/// Different resource types of a specific API. 
pub trait Resources:          Clone + Hash + Debug + Eq + PartialEq + Any {
    type Buffer:              Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type Shader:              Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type RenderPass:          Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type PipelineLayout:      Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type PipelineStateObject: Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type Image:               Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type ShaderResourceView:  Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type UnorderedAccessView: Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type RenderTargetView:    Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type DepthStencilView:    Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type Sampler:             Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
}

/// Different types of a specific API.
pub trait Backend {
    type CommandBuffer;
    // TODO: probably need to split this into multiple subqueue types (rendering, compute, transfer/copy)
    // Vulkan allows multiple combinations of these 3
    // D3D12 has a 3D queue which supports all 3 types, Compute queue with compute and transfer support and a Copy queue
    // Older APIs don't have the concept of queues anyway
    // Metal ?
    type CommandQueue: CommandQueue;
    type Device: Device;
    type Instance: Instance;
    type PhysicalDevice: PhysicalDevice;
    type Resources: Resources;
    type Surface: Surface;
    type SwapChain: SwapChain;
}
