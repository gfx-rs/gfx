/*! Command queues.

    Queues are the execution paths of the graphical processing units. These process
    submitted commands buffers.

    There are different types of queues, which can only handle associated command buffers.
    `CommandQueue<B, C>` has the capability defined by `C`: graphics, compute and transfer.
!*/

pub mod capability;
pub mod family;
pub mod submission;

use Backend;
use std::marker::PhantomData;

pub use self::capability::{
    Capability, Supports,
    Compute, Graphics, General, Transfer,
};
pub use self::family::{
    QueueFamily, QueueFamilyId, QueueGroup, Queues,
};
pub use self::submission::{RawSubmission, Submission};

///
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum QueueType {
    ///
    General,
    ///
    Graphics,
    ///
    Compute,
    ///
    Transfer,
}

/// `RawCommandQueue` are abstractions to the internal GPU execution engines.
/// Commands are executed on the the device by submitting command buffers to queues.
pub trait RawCommandQueue<B: Backend> {
    /// Submit command buffers to queue for execution.
    /// `fence` will be signalled after submission and _must_ be unsignalled.
    ///
    /// Unsafe because it's not checked that the queue can process the submitted command buffers.
    /// Trying to submit compute commands to a graphics queue will result in undefined behavior.
    /// Each queue implements safe wrappers according to their supported functionalities!
    unsafe fn submit_raw(&mut self, RawSubmission<B>, Option<&B::Fence>);
}

/// Stronger-typed and safer `CommandQueue` wraps around `RawCommandQueue`.
pub struct CommandQueue<B: Backend, C>(B::CommandQueue, PhantomData<C>);

impl<B: Backend, C> CommandQueue<B, C> {
    /// Get a reference to the raw command queue
    pub fn as_raw(&self) -> &B::CommandQueue {
        &self.0
    }

    /// Get a mutable reference to the raw command queue
    pub fn as_mut(&mut self) -> &mut B::CommandQueue {
        &mut self.0
    }

    ///
    pub fn submit<D>(&mut self,
        submission: Submission<B, D>,
        fence: Option<&B::Fence>,
    ) where
        C: Supports<D>
    {
        unsafe {
            self.0.submit_raw(submission.as_raw(), fence)
        }
    }
}
