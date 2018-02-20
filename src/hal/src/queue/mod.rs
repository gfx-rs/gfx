/*! Command queues.

    Queues are the execution paths of the graphical processing units. These process
    submitted commands buffers.

    There are different types of queues, which can only handle associated command buffers.
    `CommandQueue<B, C>` has the capability defined by `C`: graphics, compute and transfer.
!*/

pub mod capability;
pub mod family;
pub mod submission;

use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;

use error::HostExecutionError;
use Backend;

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
    unsafe fn submit_raw<IC>(&mut self, RawSubmission<B, IC>, Option<&B::Fence>)
    where
        IC: IntoIterator,
        IC::Item: Borrow<B::CommandBuffer>;

    ///
    fn present<IS, IW>(&mut self, swapchains: IS, wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<B::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<B::Semaphore>;

    /// Wait for the queue to idle.
    fn wait_idle(&self) -> Result<(), HostExecutionError>;
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
            self.0.submit_raw(submission.to_raw(), fence)
        }
    }

    ///
    pub fn present<IS, IW>(&mut self, swapchains: IS, wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<B::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<B::Semaphore>
    {
        self.0.present(swapchains, wait_semaphores)
    }

    /// Wait for the queue to idle.
    pub fn wait_idle(&self) -> Result<(), HostExecutionError> {
        self.0.wait_idle()
    }
}
