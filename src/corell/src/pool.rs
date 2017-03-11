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

use std::ops::DerefMut;
use {CommandPool, CommandQueue};

pub trait GeneralPoolSupport { type Queue: CommandQueue; }
pub trait ComputePoolSupport { type Queue: CommandQueue; }
pub trait GraphicsPoolSupport { type Queue: CommandQueue; }
pub trait TransferPoolSupport { type Queue: CommandQueue; }

/// General command pool can allocate general command buffers.
pub struct GeneralCommandPool<P: CommandPool>(P);
impl<P: CommandPool> GeneralCommandPool<P> { 
    pub fn from_queue<Q: GeneralPoolSupport + DerefMut<Target=<P as CommandPool>::Q>>(mut queue: &mut Q, capacity: usize) -> Self {
        GeneralCommandPool(P::from_queue(&mut queue, capacity))
    }
    pub fn acquire_command_buffer(&mut self) -> &mut <<P as CommandPool>::Q as CommandQueue>::GeneralCommandBuffer {
        unimplemented!()
    }
}

/// Graphics command pool can allocate graphics command buffers.
pub struct GraphicsCommandPool<P: CommandPool>(P);
impl<P: CommandPool> GraphicsCommandPool<P> { 
    pub fn from_queue<Q: GraphicsPoolSupport + DerefMut<Target=<P as CommandPool>::Q>>(mut queue: &mut Q, capacity: usize) -> Self {
        GraphicsCommandPool(P::from_queue(&mut queue, capacity))
    }
    pub fn acquire_command_buffer(&mut self) -> &mut <<P as CommandPool>::Q as CommandQueue>::GraphicsCommandBuffer {
        unimplemented!()
    }
}

/// Compute command pool can allocate compute command buffers.
pub struct ComputeCommandPool<P: CommandPool>(P);
impl<P: CommandPool> ComputeCommandPool<P> { 
    pub fn from_queue<Q: ComputePoolSupport + DerefMut<Target=<P as CommandPool>::Q>>(mut queue: &mut Q, capacity: usize) -> Self {
        ComputeCommandPool(P::from_queue(&mut queue, capacity))
    }
    pub fn acquire_command_buffer(&mut self) -> &mut <<P as CommandPool>::Q as CommandQueue>::ComputeCommandBuffer {
        unimplemented!()
    }
}

/// Transfer command pool can allocate transfer command buffers.
pub struct TransferCommandPool<P: CommandPool>(P);
impl<P: CommandPool> TransferCommandPool<P> { 
    pub fn from_queue<Q: TransferPoolSupport + DerefMut<Target=<P as CommandPool>::Q>>(mut queue: &mut Q, capacity: usize) -> Self {
        TransferCommandPool(P::from_queue(&mut queue, capacity))
    }
    pub fn acquire_command_buffer(&mut self) -> &mut <<P as CommandPool>::Q as CommandQueue>::TransferCommandBuffer {
        unimplemented!()
    }
}

/// Subpass command pool can allocate subpass command buffers.
pub struct SubpassCommandPool<P: CommandPool>(P);
impl<P: CommandPool> SubpassCommandPool<P> { 
    pub fn from_queue<Q: GraphicsPoolSupport + DerefMut<Target=<P as CommandPool>::Q>>(mut queue: &mut Q, capacity: usize) -> Self {
        SubpassCommandPool(P::from_queue(&mut queue, capacity))
    }
    pub fn acquire_command_buffer(&mut self) -> &mut <<P as CommandPool>::Q as CommandQueue>::SubpassCommandBuffer {
        unimplemented!()
    }
}
