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

/// General information about a queue family, available upon adapter discovery.
/// Not useful after an adapter opened and turned into `Gpu`.
///
/// *Note*: A backend can expose multiple queue families with the same properties.
pub trait ProtoQueueFamily: Debug {
    /// Returns the type of queues.
    fn queue_type(&self) -> QueueType;
    /// Returns maximum number of queues created from this family.
    fn max_queues(&self) -> usize;
}

/// `RawQueueFamily` denotes a group of command queues provided by the backend
/// with the same properties/type.
pub struct RawQueueFamily<B: Backend> {
    prototype: B::ProtoQueueFamily,
    queues: Vec<B::CommandQueue>,
}

//TODO: this is not a very sound structure, unfortunately.
impl<B: Backend> RawQueueFamily<B> {
    #[doc(hidden)]
    pub fn new(prototype: B::ProtoQueueFamily) -> Self {
        RawQueueFamily {
            prototype,
            queues: Vec::new(),
        }
    }
    #[doc(hidden)]
    pub fn add_queue(&mut self, queue: B::CommandQueue) {
        assert!(self.queues.len() < self.prototype.max_queues());
        self.queues.push(queue);
    }
    ///
    pub fn prototype(&self) -> &B::ProtoQueueFamily {
        &self.prototype
    }
}

/// Stronger-typed queue family.
pub struct QueueFamily<B: Backend, C> {
    pub(crate) prototype: B::ProtoQueueFamily,
    /// Command queues created in this family.
    pub queues: Vec<CommandQueue<B, C>>,
}

impl<B: Backend, C: Capability> QueueFamily<B, C> {
    /// Create a new strongly typed queue family from a raw one.
    ///
    /// *Note*: panics if the family doesn't expose required
    /// queue capabilities.
    pub fn new(raw: RawQueueFamily<B>) -> Self {
        assert!(C::supported_by(raw.prototype.queue_type()));
        QueueFamily {
            prototype: raw.prototype,
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
