/*! Command queues.

    Queues are the execution paths of the graphical processing units. These process
    submitted commands buffers.

    There are different types of queues, which can only handle associated command buffers.
    `CommandQueue<B, C>` has the capability defined by `C`: graphics, compute and transfer.
!*/

pub mod capability;
pub mod submission;

use Backend;
use std::fmt::Debug;
use std::marker::PhantomData;

pub use self::capability::{
    Capability, Supports,
    Compute, Graphics, General, Transfer,
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

/// General information about a queue family, available upon adapter discovery.
///
/// *Note*: A backend can expose multiple queue families with the same properties.
pub trait QueueFamily: Debug {
    /// Returns the type of queues.
    fn queue_type(&self) -> QueueType;
    /// Returns maximum number of queues created from this family.
    fn max_queues(&self) -> usize;
    /// Returns true if the queue supports graphics operations.
    fn supports_graphics(&self) -> bool {
        Graphics::supported_by(self.queue_type())
    }
    /// Returns true if the queue supports graphics operations.
    fn supports_compute(&self) -> bool {
        Compute::supported_by(self.queue_type())
    }
}

/// `RawQueueGroup` denotes a group of command queues provided by the backend
/// with the same properties/type.
pub struct RawQueueGroup<B: Backend> {
    family: B::QueueFamily,
    queues: Vec<B::CommandQueue>,
}

//TODO: this is not a very sound structure, unfortunately.
impl<B: Backend> RawQueueGroup<B> {
    #[doc(hidden)]
    pub fn new(family: B::QueueFamily) -> Self {
        RawQueueGroup {
            family,
            queues: Vec::new(),
        }
    }
    #[doc(hidden)]
    pub fn add_queue(&mut self, queue: B::CommandQueue) {
        assert!(self.queues.len() < self.family.max_queues());
        self.queues.push(queue);
    }
    ///
    pub fn family(&self) -> &B::QueueFamily {
        &self.family
    }
}

/// Stronger-typed queue family.
pub struct QueueGroup<B: Backend, C> {
    pub(crate) family: B::QueueFamily,
    /// Command queues created in this family.
    pub queues: Vec<CommandQueue<B, C>>,
}

impl<B: Backend, C: Capability> QueueGroup<B, C> {
    /// Create a new strongly typed queue group from a raw one.
    ///
    /// # Panics
    ///
    /// Panics if the family doesn't expose required queue capabilities.
    pub fn new(raw: RawQueueGroup<B>) -> Self {
        assert!(C::supported_by(raw.family.queue_type()));
        QueueGroup {
            family: raw.family,
            queues: raw.queues
                .into_iter()
                .map(|q| CommandQueue(q, PhantomData))
                .collect(),
        }
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
