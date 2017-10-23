/*! Command queues.

    Queues are the execution paths of the graphical processing units. These process
    submitted commands buffers.

    There are different types of queues, which can only handle associated command buffers.
    `CommandQueue<B, C>` has the capability defined by `C`: graphics, compute and transfer.
!*/

pub mod capability;
pub mod submission;

use Backend;
use pool::{CommandPool, CommandPoolCreateFlags};
use std::marker::PhantomData;

pub use self::capability::{
    Capability, Supports,
    Compute, Graphics, General, Transfer,
};
pub use self::submission::{RawSubmission, Submission};

///
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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

/// `RawQueueFamily` denotes a group of command queues provided by the backend
/// with the same properties/type.
///
/// *Note*: A backend can expose multiple queue families with the same properties.
pub trait RawQueueFamily<B: Backend> {
    /// Returns the type of queues.
    fn queue_type(&self) -> QueueType;
    /// Returns maximum number of queues created from this family.
    fn max_queues(&self) -> usize;
    /// Creates a new command queue.
    fn create_queue(&mut self) -> B::CommandQueue;
    /// Creates a new command pool.
    fn create_pool(&mut self, CommandPoolCreateFlags) -> B::CommandPool;
}

/// Stronger-typed queue family type with a specific capacity.
/// Wraps around `RawQueueFamily`.
pub struct QueueFamily<B: Backend, C> {
    raw: B::QueueFamily,
    capacity: usize,
    _capability: PhantomData<C>,
}

impl<B: Backend, C> QueueFamily<B, C> {
    #[doc(hidden)]
    pub unsafe fn new(raw: B::QueueFamily) -> Self {
        let capacity = raw.max_queues();
        QueueFamily {
            raw,
            capacity,
            _capability: PhantomData,
        }
    }

    /// Gets a reference to the enclosed raw queue family.
    pub fn raw(&self) -> &B::QueueFamily {
        &self.raw
    }

    /// Returns the number of queues still available in this family.
    pub fn num_queues(&self) -> usize {
        self.capacity
    }

    /// Creates a stronger-typed command queue.
    pub fn create_queue(&mut self) -> CommandQueue<B, C> {
        assert_ne!(self.capacity, 0);
        self.capacity -= 1;
        CommandQueue(self.raw.create_queue(), PhantomData)
    }

    /// Creates a stronger-typed command pool.
    pub fn create_pool(
        &mut self, max_buffers: usize, flags: CommandPoolCreateFlags
    ) -> CommandPool<B, C> {
        let raw = self.raw.create_pool(flags);
        unsafe { CommandPool::new(raw, max_buffers) }
    }
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
