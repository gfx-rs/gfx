//! Command queues.
//!
//! Queues are the execution paths of the graphical processing units. These process
//! submitted commands buffers.
//!
//! There are different types of queues, which can only handle associated command buffers.
//! `CommandQueue<B, C>` has the capability defined by `C`: graphics, compute and transfer.

pub mod capability;
pub mod family;

use std::any::Any;
use std::borrow::Borrow;
use std::marker::PhantomData;

use command::{Primary, Submittable};
use error::HostExecutionError;
use pso;
use window::SwapImageIndex;
use Backend;

pub use self::capability::{
    Capability, Supports,
    Compute, Graphics, General, Transfer,
};
pub use self::family::{
    QueueFamily, QueueFamilyId, QueueGroup, Queues,
};


/// The type of the queue, an enum encompassing `queue::Capability`
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum QueueType {
    /// Supports all operations.
    General,
    /// Only supports graphics and transfer operations.
    Graphics,
    /// Only supports compute and transfer operations.
    Compute,
    /// Only supports transfer operations.
    Transfer,
}

/// Submission information for a command queue.
pub struct Submission<'a, B: Backend + 'a, I> {
    /// Command buffers to submit.
    pub command_buffers: I,
    /// Semaphores to wait being signalled before submission.
    pub wait_semaphores: &'a [(&'a B::Semaphore, pso::PipelineStage)],
    /// Semaphores which get signalled after submission.
    pub signal_semaphores: &'a [&'a B::Semaphore],
}

/// `RawCommandQueue` are abstractions to the internal GPU execution engines.
/// Commands are executed on the the device by submitting command buffers to queues.
pub trait RawCommandQueue<B: Backend>: Any + Send + Sync {
    /// Submit command buffers to queue for execution.
    /// `fence` will be signalled after submission and _must_ be unsignalled.
    ///
    /// Unsafe because it's not checked that the queue can process the submitted command buffers.
    /// Trying to submit compute commands to a graphics queue will result in undefined behavior.
    /// Each queue implements safe wrappers according to their supported functionalities!
    unsafe fn submit<'a, T, I>(
        &mut self, submission: Submission<'a, B, I>, fence: Option<&B::Fence>
    ) where
        T: 'a + Borrow<B::CommandBuffer>,
        I: IntoIterator<Item = &'a T>;

    /// Presents the result of the queue to the given swapchains, after waiting on all the
    /// semaphores given in `wait_semaphores`. A given swapchain must not appear in this
    /// list more than once.
    ///
    /// Unsafe for the same reasons as `submit()`.
    fn present<IS, S, IW>(&mut self, swapchains: IS, wait_semaphores: IW) -> Result<(), ()>
    where
        Self: Sized,
        IS: IntoIterator<Item = (S, SwapImageIndex)>,
        S: Borrow<B::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<B::Semaphore>;

    /// Wait for the queue to idle.
    fn wait_idle(&self) -> Result<(), HostExecutionError>;
}

/// Stronger-typed and safer `CommandQueue` wraps around `RawCommandQueue`.
pub struct CommandQueue<B: Backend, C>(B::CommandQueue, PhantomData<C>);

impl<B: Backend, C: Capability> CommandQueue<B, C> {
    /// Create typed command queue from raw.
    ///
    /// # Safety
    ///
    /// `<C as Capability>::supported_by(queue_type)` must return true
    /// for `queue_type` being the type this `raw` queue.
    pub unsafe fn new(raw: B::CommandQueue) -> Self {
        CommandQueue(raw, PhantomData)
    }

    /// Get a reference to the raw command queue
    pub fn as_raw(&self) -> &B::CommandQueue {
        &self.0
    }

    /// Get a mutable reference to the raw command queue
    pub fn as_raw_mut(&mut self) -> &mut B::CommandQueue {
        &mut self.0
    }

    /// Downgrade a typed command queue to untyped one.
    pub fn into_raw(self) -> B::CommandQueue {
        self.0
    }

    /// Submits the submission command buffers to the queue for execution.
    /// `fence` will be signalled after submission and _must_ be unsignalled.
    pub fn submit<'a, T, I>(&mut self,
        submission: Submission<'a, B, I>,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Submittable<B, C, Primary>,
        I: IntoIterator<Item = &'a T>,
    {
        unsafe {
            self.0.submit(submission, fence)
        }
    }

    /// Presents the result of the queue to the given swapchains, after waiting on all the
    /// semaphores given in `wait_semaphores`. A given swapchain must not appear in this
    /// list more than once.
    pub fn present<IS, S, IW>(&mut self, swapchains: IS, wait_semaphores: IW) -> Result<(), ()>
    where
        IS: IntoIterator<Item = (S, SwapImageIndex)>,
        S: Borrow<B::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<B::Semaphore>
    {
        self.0.present(swapchains, wait_semaphores)
    }

    /// Wait for the queue to idle.
    pub fn wait_idle(&self) -> Result<(), HostExecutionError> {
        self.0.wait_idle()
    }

    /// Downgrade a command queue to a lesser capability type.
    pub fn downgrade<D>(&mut self) -> &mut CommandQueue<B, D>
    where
        C: Supports<D>,
    {
        unsafe {
            ::std::mem::transmute(self)
        }
    }
}
