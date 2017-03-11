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

//! Queue and Command Pool handling.
//!
//! There are different types of queues, which can create and take specific command buffers.

use std::ops::{Deref, DerefMut};
use {CommandQueue, Resources};

pub trait GeneralPoolSupport { type Queue: CommandQueue; }
pub trait ComputePoolSupport { type Queue: CommandQueue; }
pub trait GraphicsPoolSupport { type Queue: CommandQueue; }
pub trait TransferPoolSupport { type Queue: CommandQueue; }

/// General command queue, which can execute graphics, compute and transfer command buffers.
pub struct GeneralQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GeneralQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GeneralQueue(queue)
    }

    pub fn submit_general(&mut self, cmd_buffer: &<Q as CommandQueue>::GeneralCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_graphics(&mut self, cmd_buffer: &<Q as CommandQueue>::GraphicsCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_compute(&mut self, cmd_buffer: &<Q as CommandQueue>::ComputeCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_tranfer(&mut self, cmd_buffer: &<Q as CommandQueue>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> GeneralPoolSupport for GeneralQueue<Q> { type Queue = Q; }
impl<Q: CommandQueue> ComputePoolSupport for GeneralQueue<Q> { type Queue = Q; }
impl<Q: CommandQueue> GraphicsPoolSupport for GeneralQueue<Q> { type Queue = Q; }
impl<Q: CommandQueue> TransferPoolSupport for GeneralQueue<Q> { type Queue = Q; }

impl<Q: CommandQueue> Deref for GeneralQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for GeneralQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

impl<Q: CommandQueue> Into<GraphicsQueue<Q>> for GeneralQueue<Q> {
    fn into(self) -> GraphicsQueue<Q> {
        GraphicsQueue(self.0)
    }
}
impl<Q: CommandQueue> Into<ComputeQueue<Q>> for GeneralQueue<Q> {
    fn into(self) -> ComputeQueue<Q> {
        ComputeQueue(self.0)
    }
}
impl<Q: CommandQueue> Into<TransferQueue<Q>> for GeneralQueue<Q> {
    fn into(self) -> TransferQueue<Q> {
        TransferQueue(self.0)
    }
}

/// Graphics command queue, which can execute graphics and transfer command buffers.
pub struct GraphicsQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GraphicsQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GraphicsQueue(queue)
    }

    pub fn submit_graphics(&mut self, cmd_buffer: &<Q as CommandQueue>::GraphicsCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_tranfer(&mut self, cmd_buffer: &<Q as CommandQueue>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> GraphicsPoolSupport for GraphicsQueue<Q> { type Queue = Q; }
impl<Q: CommandQueue> TransferPoolSupport for GraphicsQueue<Q> { type Queue = Q; }

impl<Q: CommandQueue> Deref for GraphicsQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for GraphicsQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

impl<Q: CommandQueue> Into<TransferQueue<Q>> for GraphicsQueue<Q> {
    fn into(self) -> TransferQueue<Q> {
        TransferQueue(self.0)
    }
}

/// Compute command queue, which can execute compute and transfer command buffers.
pub struct ComputeQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> ComputeQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        ComputeQueue(queue)
    }

    pub fn submit_compute(&mut self, cmd_buffer: &<Q as CommandQueue>::ComputeCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_tranfer(&mut self, cmd_buffer: &<Q as CommandQueue>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> ComputePoolSupport for ComputeQueue<Q> { type Queue = Q; }
impl<Q: CommandQueue> TransferPoolSupport for ComputeQueue<Q> { type Queue = Q; }

impl<Q: CommandQueue> Deref for ComputeQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for ComputeQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

impl<Q: CommandQueue> Into<TransferQueue<Q>> for ComputeQueue<Q> {
    fn into(self) -> TransferQueue<Q> {
        TransferQueue(self.0)
    }
}

/// Transfer command queue, which can execute transfer command buffers.
pub struct TransferQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> TransferQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        TransferQueue(queue)
    }

    pub fn submit_tranfer(&mut self, cmd_buffer: &<Q as CommandQueue>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> TransferPoolSupport for TransferQueue<Q> { type Queue = Q; }

impl<Q: CommandQueue> Deref for TransferQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for TransferQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

/// `CommandPool` can allocate command buffers of a specific type only.
/// The allocated command buffers are associated with the creating command queue.
pub trait CommandPool {
    type Q: CommandQueue;

    fn from_queue(queue: &mut Self::Q, capacity: usize) -> Self;
    fn reset(&mut self);
    fn reserve(&mut self, additional: usize);
}

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
