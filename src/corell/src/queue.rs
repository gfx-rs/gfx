//! Command queue types.
//!
//! There are different types of queues, which can create and submit associated command buffers.

use std::ops::{Deref, DerefMut};
use {CommandQueue, QueueSubmit, Resources};

/// General command queue, which can execute graphics, compute and transfer command buffers.
pub struct GeneralQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GeneralQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GeneralQueue(queue)
    }

    pub fn submit_general(&mut self, submit: &[QueueSubmit<Q::GeneralCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    pub fn submit_graphics(&mut self, submit: &[QueueSubmit<Q::GraphicsCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    pub fn submit_compute(&mut self, submit: &[QueueSubmit<Q::ComputeCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    pub fn submit_tranfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
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

    pub fn submit_graphics(&mut self, submit: &[QueueSubmit<Q::GraphicsCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    pub fn submit_tranfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
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

    pub fn submit_compute(&mut self, submit: &[QueueSubmit<Q::ComputeCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
        unsafe { self.submit(submit, fence) }
    }
    pub fn submit_tranfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
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

    pub fn submit_tranfer(&mut self, submit: &[QueueSubmit<Q::TransferCommandBuffer, Q::R>], fence: Option<&mut <Q::R as Resources>::Fence>) {
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
