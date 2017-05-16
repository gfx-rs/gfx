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

extern crate gfx_corell as core;
extern crate winit;

extern crate metal_rs as metal;

mod command;
mod factory;
mod native;

pub use command::{QueueFamily, CommandQueue, CommandPool, RenderPassInlineEncoder};
pub use factory::{Factory};

pub type GraphicsCommandPool = CommandPool;

use core::format;
use metal::*;

pub struct Instance {
}

pub struct Adapter {
    device: MTLDevice
}

impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe { self.device.release(); }
    }
}

pub struct Surface {
    layer: CAMetalLayer
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.layer.release(); }
    }
}

pub struct SwapChain {
}

#[derive(Debug, Clone, Hash)]
pub enum Resources {}

impl core::Instance for Instance {
    type Adapter = Adapter;
    type Surface = Surface;
    type Window = winit::Window;

    fn create() -> Self {
        Instance {}
    }

    fn enumerate_adapters(&self) -> Vec<Self::Adapter> {
        unimplemented!()
    }

    fn create_surface(&self, window: &winit::Window) -> Self::Surface {
        unimplemented!()
    }
}

impl core::Adapter for Adapter {
    type CommandQueue = CommandQueue;
    type QueueFamily = QueueFamily;
    type Factory = Factory;
    type Resources = Resources;

    fn open<'a, I>(&self, queue_descs: I) -> core::Device<Self::Resources, Self::Factory, Self::CommandQueue>
        where I: ExactSizeIterator<Item=(&'a Self::QueueFamily, u32)> 
    {
        unimplemented!()
    }

    fn get_info(&self) -> &core::AdapterInfo {
        unimplemented!()
    }

    fn get_queue_families(&self) -> std::slice::Iter<Self::QueueFamily> {
        unimplemented!()
    }
}

impl core::Surface for Surface {
    type Queue = CommandQueue;
    type SwapChain = SwapChain;

    fn build_swapchain<T: format::RenderFormat>(&self, queue: &CommandQueue) -> SwapChain {
        unimplemented!()
    }
}

impl core::SwapChain for SwapChain {
    type R = Resources;
    type Image = native::Image;

    fn get_images(&mut self) -> &[native::Image] {
        unimplemented!()
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<Resources>) -> core::Frame {
        unimplemented!()
    }

    fn present(&mut self) {
    }
}

impl core::Resources for Resources {
    type ShaderLib = native::ShaderLib;
    type RenderPass = native::RenderPass;
    type PipelineLayout = native::PipelineLayout;
    type FrameBuffer = native::FrameBuffer;
    type GraphicsPipeline = native::GraphicsPipeline;
    type ComputePipeline = native::ComputePipeline;
    type UnboundBuffer = native::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = native::UnboundImage;
    type Image = native::Image;
    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;
    type Sampler = native::Sampler;
    type Semaphore = native::Semaphore;
    type Fence = native::Fence;
    type Heap = native::Heap;
    type Mapping = native::Mapping;
    type DescriptorHeap = native::DescriptorHeap;
    type DescriptorSetPool = native::DescriptorSetPool;
    type DescriptorSet = native::DescriptorSet;
    type DescriptorSetLayout = native::DescriptorSetLayout;

}
