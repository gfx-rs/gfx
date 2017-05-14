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

//! Command queue types.
//!
//! There are different types of queues, which can create and submit associated command buffers.

use std::ops::{Deref, DerefMut};
use {CommandQueue, QueueSubmit, Resources};
use command::Submit;

/// General command queue, which can execute graphics, compute and transfer command buffers.
pub struct GeneralQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GeneralQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GeneralQueue(queue)
    }

    /// Submit general command buffers for execution.
    pub fn submit_general(&mut self, submit: &[QueueSubmit<Q::GeneralCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    /// Submit graphics command buffers for execution.
    pub fn submit_graphics(&mut self, submit: &[QueueSubmit<Q::GraphicsCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    /// Submit compute command buffers for execution.
    pub fn submit_compute(&mut self, submit: &[QueueSubmit<Q::ComputeCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
}

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

impl<Q: CommandQueue> From<GeneralQueue<Q>> for GraphicsQueue<Q> {
    fn from(queue: GeneralQueue<Q>) -> GraphicsQueue<Q> {
        GraphicsQueue(queue.0)
    }
}
impl<Q: CommandQueue> From<GeneralQueue<Q>> for ComputeQueue<Q> {
    fn from(queue: GeneralQueue<Q>) -> ComputeQueue<Q> {
        ComputeQueue(queue.0)
    }
}
impl<Q: CommandQueue> From<GeneralQueue<Q>> for TransferQueue<Q> {
    fn from(queue: GeneralQueue<Q>) -> TransferQueue<Q> {
        TransferQueue(queue.0)
    }
}

impl<'a, Q: CommandQueue> From<&'a GeneralQueue<Q>> for &'a GraphicsQueue<Q> {
    fn from(queue: &'a GeneralQueue<Q>) -> &'a GraphicsQueue<Q> {
        unsafe { &*(queue as *const _ as *const GraphicsQueue<Q>) }
    }
}
impl<'a, Q: CommandQueue> From<&'a GeneralQueue<Q>> for &'a ComputeQueue<Q> {
    fn from(queue: &'a GeneralQueue<Q>) -> &'a ComputeQueue<Q> {
        unsafe { &*(queue as *const _ as *const ComputeQueue<Q>) }
    }
}
impl<'a, Q: CommandQueue> From<&'a GeneralQueue<Q>> for &'a TransferQueue<Q> {
    fn from(queue: &'a GeneralQueue<Q>) -> &'a TransferQueue<Q> {
        unsafe { &*(queue as *const _ as *const TransferQueue<Q>) }
    }
}

impl<'a, Q: CommandQueue> From<&'a mut GeneralQueue<Q>> for &'a mut GraphicsQueue<Q> {
    fn from(queue: &'a mut GeneralQueue<Q>) -> &'a mut GraphicsQueue<Q> {
        unsafe { &mut *(queue as *mut _ as *mut GraphicsQueue<Q>) }
    }
}
impl<'a, Q: CommandQueue> From<&'a mut GeneralQueue<Q>> for &'a mut ComputeQueue<Q> {
    fn from(queue: &'a mut GeneralQueue<Q>) -> &'a mut ComputeQueue<Q> {
        unsafe { &mut *(queue as *mut _ as *mut ComputeQueue<Q>) }
    }
}
impl<'a, Q: CommandQueue> From<&'a mut GeneralQueue<Q>> for &'a mut TransferQueue<Q> {
    fn from(queue: &'a mut GeneralQueue<Q>) -> &'a mut TransferQueue<Q> {
        unsafe { &mut *(queue as *mut _ as *mut TransferQueue<Q>) }
    }
}

/// Graphics command queue, which can execute graphics and transfer command buffers.
pub struct GraphicsQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GraphicsQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GraphicsQueue(queue)
    }

    /// Submit graphics command buffers for execution.
    pub fn submit_graphics(&mut self, submit: &[QueueSubmit<Q::GraphicsCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
}

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

impl<Q: CommandQueue> From<GraphicsQueue<Q>> for TransferQueue<Q> {
    fn from(queue: GraphicsQueue<Q>) -> TransferQueue<Q> {
        TransferQueue(queue.0)
    }
}

/// Compute command queue, which can execute compute and transfer command buffers.
pub struct ComputeQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> ComputeQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        ComputeQueue(queue)
    }

    /// Submit compute command buffers for execution.
    pub fn submit_compute(&mut self, submit: &[QueueSubmit<Q::ComputeCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
}

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

impl<Q: CommandQueue> From<ComputeQueue<Q>> for TransferQueue<Q> {
    fn from(queue: ComputeQueue<Q>) -> TransferQueue<Q> {
        TransferQueue(queue.0)
    }
}

/// Transfer command queue, which can execute transfer command buffers.
pub struct TransferQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> TransferQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        TransferQueue(queue)
    }

    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
}

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
