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

use std::borrow::{Borrow, BorrowMut};
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use {CommandQueue, QueueSubmit, Resources};
use command::Submit;

///
pub trait Repr<Q: CommandQueue> { }

///
impl<Q: CommandQueue> Repr<Q> for Q {}

impl<'a, Q: CommandQueue> Repr<Q> for &'a Q {}
impl<'a, Q: CommandQueue> Repr<Q> for &'a mut Q {}

/// General command queue, which can execute graphics, compute and transfer command buffers.
pub struct GeneralQueueBase<Q: CommandQueue, R: Repr<Q>>(R, PhantomData<Q>);
impl<Q: CommandQueue> GeneralQueueBase<Q, Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GeneralQueueBase(queue, PhantomData)
    }

    ///
    pub fn as_ref(&self) -> GeneralQueueBase<Q, &Q> {
        GeneralQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> GeneralQueueBase<Q, &mut Q> {
        GeneralQueueBase(&mut self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> GeneralQueueBase<Q, &'a Q> {
    ///
    pub fn as_ref(&self) -> GeneralQueueBase<Q, &Q> {
        GeneralQueueBase(&self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> GeneralQueueBase<Q, &'a mut Q> {
    ///
    pub fn as_ref(&self) -> GeneralQueueBase<Q, &Q> {
        GeneralQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> GeneralQueueBase<Q, &mut Q> {
        GeneralQueueBase(&mut self.0, PhantomData)
    }
}

///
pub type GeneralQueue<Q: CommandQueue> = GeneralQueueBase<Q, Q>;

impl<'a, Q: CommandQueue> GeneralQueueBase<Q, &'a mut Q> {
    /// Submit general command buffers for execution.
    pub fn submit_general(&mut self, submit: &[QueueSubmit<Q::GeneralCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
    /// Submit graphics command buffers for execution.
    pub fn submit_graphics(&mut self, submit: &[QueueSubmit<Q::GraphicsCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
    /// Submit compute command buffers for execution.
    pub fn submit_compute(&mut self, submit: &[QueueSubmit<Q::ComputeCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
}

impl<Q: CommandQueue> Deref for GeneralQueueBase<Q, Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> Deref for GeneralQueueBase<Q, &'a Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> Deref for GeneralQueueBase<Q, &'a mut Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<Q: CommandQueue> DerefMut for GeneralQueueBase<Q, Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<'a, Q: CommandQueue> DerefMut for GeneralQueueBase<Q, &'a mut Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<Q: CommandQueue> Borrow<Q> for GeneralQueueBase<Q, Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for GeneralQueueBase<Q, &'a Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for GeneralQueueBase<Q, &'a mut Q> {
    fn borrow(&self) -> &Q { &self.0 }
}

impl<Q: CommandQueue> BorrowMut<Q> for GeneralQueueBase<Q, Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}
impl<'a, Q: CommandQueue> BorrowMut<Q> for GeneralQueueBase<Q, &'a mut Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}

impl<Q: CommandQueue, R: Repr<Q>> From<GeneralQueueBase<Q, R>> for GraphicsQueueBase<Q, R> {
    fn from(queue: GeneralQueueBase<Q, R>) -> GraphicsQueueBase<Q, R> {
        GraphicsQueueBase(queue.0, PhantomData)
    }
}
impl<Q: CommandQueue, R: Repr<Q>> From<GeneralQueueBase<Q, R>> for ComputeQueueBase<Q, R> {
    fn from(queue: GeneralQueueBase<Q, R>) -> ComputeQueueBase<Q, R> {
        ComputeQueueBase(queue.0, PhantomData)
    }
}
impl<Q: CommandQueue, R: Repr<Q>> From<GeneralQueueBase<Q, R>> for TransferQueueBase<Q, R> {
    fn from(queue: GeneralQueueBase<Q, R>) -> TransferQueueBase<Q, R> {
        TransferQueueBase(queue.0, PhantomData)
    }
}

/// Graphics command queue, which can execute graphics and transfer command buffers.
pub struct GraphicsQueueBase<Q: CommandQueue, R: Repr<Q>>(R, PhantomData<Q>);
impl<Q: CommandQueue> GraphicsQueueBase<Q, Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GraphicsQueueBase(queue, PhantomData)
    }

    ///
    pub fn as_ref(&self) -> GraphicsQueueBase<Q, &Q> {
        GraphicsQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> GraphicsQueueBase<Q, &mut Q> {
        GraphicsQueueBase(&mut self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> GraphicsQueueBase<Q, &'a Q> {
    ///
    pub fn as_ref(&self) -> GraphicsQueueBase<Q, &Q> {
        GraphicsQueueBase(&self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> GraphicsQueueBase<Q, &'a mut Q> {
    ///
    pub fn as_ref(&self) -> GraphicsQueueBase<Q, &Q> {
        GraphicsQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> GraphicsQueueBase<Q, &mut Q> {
        GraphicsQueueBase(&mut self.0, PhantomData)
    }
}

///
pub type GraphicsQueue<Q: CommandQueue> = GraphicsQueueBase<Q, Q>;

impl<'a, Q: CommandQueue> GraphicsQueueBase<Q, &'a mut Q> {
    /// Submit graphics command buffers for execution.
    pub fn submit_graphics(&mut self, submit: &[QueueSubmit<Q::GraphicsCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
}

impl<Q: CommandQueue> Deref for GraphicsQueueBase<Q, Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<Q: CommandQueue> DerefMut for GraphicsQueueBase<Q, Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<'a, Q: CommandQueue> Deref for GraphicsQueueBase<Q, &'a Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> Deref for GraphicsQueueBase<Q, &'a mut Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> DerefMut for GraphicsQueueBase<Q, &'a mut Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<Q: CommandQueue> Borrow<Q> for GraphicsQueueBase<Q, Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for GraphicsQueueBase<Q, &'a Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for GraphicsQueueBase<Q, &'a mut Q> {
    fn borrow(&self) -> &Q { &self.0 }
}

impl<Q: CommandQueue> BorrowMut<Q> for GraphicsQueueBase<Q, Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}
impl<'a, Q: CommandQueue> BorrowMut<Q> for GraphicsQueueBase<Q, &'a mut Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}

impl<Q: CommandQueue, R: Repr<Q>> From<GraphicsQueueBase<Q, R>> for TransferQueueBase<Q, R> {
    fn from(queue: GraphicsQueueBase<Q, R>) -> TransferQueueBase<Q, R> {
        TransferQueueBase(queue.0, PhantomData)
    }
}

/// Compute command queue, which can execute compute and transfer command buffers.
pub struct ComputeQueueBase<Q: CommandQueue, R: Repr<Q>>(R, PhantomData<Q>);
impl<Q: CommandQueue> ComputeQueueBase<Q, Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        ComputeQueueBase(queue, PhantomData)
    }

     ///
    pub fn as_ref(&self) -> ComputeQueueBase<Q, &Q> {
        ComputeQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> ComputeQueueBase<Q, &mut Q> {
        ComputeQueueBase(&mut self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> ComputeQueueBase<Q, &'a Q> {
    ///
    pub fn as_ref(&self) -> ComputeQueueBase<Q, &Q> {
        ComputeQueueBase(&self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> ComputeQueueBase<Q, &'a mut Q> {
    ///
    pub fn as_ref(&self) -> ComputeQueueBase<Q, &Q> {
        ComputeQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> ComputeQueueBase<Q, &mut Q> {
        ComputeQueueBase(&mut self.0, PhantomData)
    }
}

///
pub type ComputeQueue<Q: CommandQueue> = ComputeQueueBase<Q, Q>;

impl<'a, Q: CommandQueue> ComputeQueueBase<Q, &'a mut Q> {
    /// Submit compute command buffers for execution.
    pub fn submit_compute(&mut self, submit: &[QueueSubmit<Q::ComputeCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
}

impl<Q: CommandQueue> Deref for ComputeQueueBase<Q, Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<Q: CommandQueue> DerefMut for ComputeQueueBase<Q, Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<'a, Q: CommandQueue> Deref for ComputeQueueBase<Q, &'a Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> Deref for ComputeQueueBase<Q, &'a mut Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> DerefMut for ComputeQueueBase<Q, &'a mut Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<Q: CommandQueue> Borrow<Q> for ComputeQueueBase<Q, Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for ComputeQueueBase<Q, &'a Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for ComputeQueueBase<Q, &'a mut Q> {
    fn borrow(&self) -> &Q { &self.0 }
}

impl<Q: CommandQueue> BorrowMut<Q> for ComputeQueueBase<Q, Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}
impl<'a, Q: CommandQueue> BorrowMut<Q> for ComputeQueueBase<Q, &'a mut Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}

impl<Q: CommandQueue, R: Repr<Q>> From<ComputeQueueBase<Q, R>> for TransferQueueBase<Q, R> {
    fn from(queue: ComputeQueueBase<Q, R>) -> TransferQueueBase<Q, R> {
        TransferQueueBase(queue.0, PhantomData)
    }
}

/// Transfer command queue, which can execute transfer command buffers.
pub struct TransferQueueBase<Q: CommandQueue, R: Repr<Q>>(R, PhantomData<Q>);
impl<Q: CommandQueue> TransferQueueBase<Q, Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        TransferQueueBase(queue, PhantomData)
    }

     ///
    pub fn as_ref(&self) -> TransferQueueBase<Q, &Q> {
        TransferQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> TransferQueueBase<Q, &mut Q> {
        TransferQueueBase(&mut self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> TransferQueueBase<Q, &'a Q> {
    ///
    pub fn as_ref(&self) -> TransferQueueBase<Q, &Q> {
        TransferQueueBase(&self.0, PhantomData)
    }
}
impl<'a, Q: CommandQueue> TransferQueueBase<Q, &'a mut Q> {
    ///
    pub fn as_ref(&self) -> TransferQueueBase<Q, &Q> {
        TransferQueueBase(&self.0, PhantomData)
    }

    ///
    pub fn as_mut(&mut self) -> TransferQueueBase<Q, &mut Q> {
        TransferQueueBase(&mut self.0, PhantomData)
    }
}

///
pub type TransferQueue<Q: CommandQueue> = TransferQueueBase<Q, Q>;

impl<'a, Q: CommandQueue> TransferQueueBase<Q, &'a mut Q> {
    /// Submit transfer command buffers for execution.
    pub fn submit_transfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::Resources>], fence: Option<&mut <Q::Resources as Resources>::Fence>) {
        unsafe { self.0.submit(submit, fence) }
    }
}

impl<Q: CommandQueue> Deref for TransferQueueBase<Q, Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<Q: CommandQueue> DerefMut for TransferQueueBase<Q, Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<'a, Q: CommandQueue> Deref for TransferQueueBase<Q, &'a Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> Deref for TransferQueueBase<Q, &'a mut Q> {
    type Target = Q;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'a, Q: CommandQueue> DerefMut for TransferQueueBase<Q, &'a mut Q> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<Q: CommandQueue> Borrow<Q> for TransferQueueBase<Q, Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for TransferQueueBase<Q, &'a Q> {
    fn borrow(&self) -> &Q { &self.0 }
}
impl<'a, Q: CommandQueue> Borrow<Q> for TransferQueueBase<Q, &'a mut Q> {
    fn borrow(&self) -> &Q { &self.0 }
}

impl<Q: CommandQueue> BorrowMut<Q> for TransferQueueBase<Q, Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}
impl<'a, Q: CommandQueue> BorrowMut<Q> for TransferQueueBase<Q, &'a mut Q> {
    fn borrow_mut(&mut self) -> &mut Q { &mut self.0 }
}
