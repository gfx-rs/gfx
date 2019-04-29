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
use std::iter;
use std::marker::PhantomData;
use std::fmt;

use crate::command::{Primary, Submittable};
use crate::error::HostExecutionError;
use crate::pso;
use crate::window::{PresentError, Suboptimal, SwapImageIndex};
use crate::Backend;

pub use self::capability::{Capability, Compute, General, Graphics, Supports, Transfer};
pub use self::family::{QueueFamily, QueueFamilyId, QueueGroup, Queues};

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
pub struct Submission<Ic, Iw, Is> {
    /// Command buffers to submit.
    pub command_buffers: Ic,
    /// Semaphores to wait being signalled before submission.
    pub wait_semaphores: Iw,
    /// Semaphores to signal after all command buffers in the submission have finished execution.
    pub signal_semaphores: Is,
}

/// `RawCommandQueue` are abstractions to the internal GPU execution engines.
/// Commands are executed on the the device by submitting command buffers to queues.
pub trait RawCommandQueue<B: Backend>: fmt::Debug + Any + Send + Sync {
    /// Submit command buffers to queue for execution.
    /// `fence` must be in unsignalled state, and will be signalled after all command buffers in the submission have
    /// finished execution.
    ///
    /// Unsafe because it's not checked that the queue can process the submitted command buffers.
    /// Trying to submit compute commands to a graphics queue will result in undefined behavior.
    /// Each queue implements safer wrappers according to their supported functionalities!
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        submission: Submission<Ic, Iw, Is>,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Borrow<B::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>;

    /// Presents the result of the queue to the given swapchains, after waiting on all the
    /// semaphores given in `wait_semaphores`. A given swapchain must not appear in this
    /// list more than once.
    ///
    /// Unsafe for the same reasons as `submit()`.
    unsafe fn present<'a, W, Is, S, Iw>(
        &mut self,
        swapchains: Is,
        wait_semaphores: Iw,
    ) -> Result<Option<Suboptimal>, PresentError>
    where
        Self: Sized,
        W: 'a + Borrow<B::Swapchain>,
        Is: IntoIterator<Item = (&'a W, SwapImageIndex)>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item = &'a S>;

    /// Wait for the queue to idle.
    fn wait_idle(&self) -> Result<(), HostExecutionError>;
}

/// Stronger-typed and safer `CommandQueue` wraps around `RawCommandQueue`.
#[derive(Debug)]
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
    pub unsafe fn as_raw_mut(&mut self) -> &mut B::CommandQueue {
        &mut self.0
    }

    /// Downgrade a typed command queue to untyped one.
    pub fn into_raw(self) -> B::CommandQueue {
        self.0
    }

    /// Submit command buffers to queue for execution.
    /// `fence` must be in unsignalled state, and will be signalled after all command buffers in the submission have
    /// finished execution.
    pub unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        submission: Submission<Ic, Iw, Is>,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Submittable<B, C, Primary>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        self.0.submit(submission, fence)
    }

    /// Submit command buffers without any semaphore waits or signals.
    pub unsafe fn submit_nosemaphores<'a, T, I>(
        &mut self,
        command_buffers: I,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Submittable<B, C, Primary>,
        I: IntoIterator<Item = &'a T>,
    {
        let submission = Submission {
            command_buffers,
            wait_semaphores: iter::empty(),
            signal_semaphores: iter::empty(),
        };
        self.submit::<_, _, B::Semaphore, _, _>(submission, fence)
    }

    /// Presents the result of the queue to the given swapchains, after waiting on all the
    /// semaphores given in `wait_semaphores`. A given swapchain must not appear in this
    /// list more than once.
    pub unsafe fn present<'a, W, Is, S, Iw>(
        &mut self,
        swapchains: Is,
        wait_semaphores: Iw,
    ) -> Result<Option<Suboptimal>, PresentError>
    where
        W: 'a + Borrow<B::Swapchain>,
        Is: IntoIterator<Item = (&'a W, SwapImageIndex)>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item = &'a S>,
    {
        self.0.present(swapchains, wait_semaphores)
    }

    /// Wait for the queue to idle.
    pub fn wait_idle(&self) -> Result<(), HostExecutionError> {
        self.0.wait_idle()
    }

    /// Downgrade a command queue to a lesser capability type.
    pub unsafe fn downgrade<D>(&mut self) -> &mut CommandQueue<B, D>
    where
        C: Supports<D>,
    {
        ::std::mem::transmute(self)
    }
}
