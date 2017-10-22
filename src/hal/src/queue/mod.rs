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

pub use self::capability::{Compute, Graphics, General, Transfer, Supports};
pub use self::submission::{RawSubmission, Submission};

/// Queue descriptors are needed for `Gpu` creation via the `Adapter`.
pub struct QueueDescriptor<'a, B: Backend> {
    /// The requested queue family.
    pub family: &'a B::QueueFamily,
    /// Specifies the type of the queue.
    pub ty: QueueType,
    /// Specifies how many queues should be created.
    pub num_queues: u32,
}

impl<'a, B: Backend> Copy for QueueDescriptor<'a, B> {}

impl<'a, B: Backend> Clone for QueueDescriptor<'a, B> {
    fn clone(&self) -> Self {
        QueueDescriptor {
            family: self.family,
            ty: self.ty,
            num_queues: self.num_queues,
        }
    }
}

impl<'a, B: Backend> QueueDescriptor<'a, B> {
    /// Creates a new queue descriptor.
    pub fn new(family: &'a B::QueueFamily, ty: QueueType, num_queues: u32) -> Self {
        QueueDescriptor {
            family,
            ty,
            num_queues,
        }
    }
}

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

impl QueueType {
    /// Checks for graphics functionality support.
    /// Supported by general and graphics queues.
    pub fn supports_graphics(&self) -> bool {
        *self == QueueType::General || *self == QueueType::Graphics
    }

    /// Checks for graphics functionality support.
    /// Supported by general and compute queues.
    pub fn supports_compute(&self) -> bool {
        *self == QueueType::General || *self == QueueType::Compute
    }

    /// Checks for graphics functionality support.
    /// Supported by general, graphics and compute queues.
    pub fn supports_transfer(&self) -> bool { true }
}

/// `QueueFamily` denotes a group of command queues provided by the backend
/// with the same properties/type.
///
/// *Note*: A backend can expose multiple queue families with the same properties.
pub trait QueueFamily: 'static {
    /// Return the number of available queues of this family.
    // TODO: some backends like d3d12 support infinite software queues (verify)
    fn num_queues(&self) -> u32;
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
    #[doc(hidden)]
    pub unsafe fn new(raw: B::CommandQueue) -> Self {
        CommandQueue(raw, PhantomData)
    }

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

    ///
    pub fn create_general_pool(
        &self,
        capacity: usize,
        flags: CommandPoolCreateFlags,
    ) -> CommandPool<B, General>
    where
        C: Supports<General>
    {
        CommandPool::from_queue(self, capacity, flags)
    }

    ///
    pub fn create_graphics_pool(
        &self,
        capacity: usize,
        flags: CommandPoolCreateFlags,
    ) -> CommandPool<B, Graphics>
    where
        C: Supports<Graphics>
    {
        CommandPool::from_queue(self, capacity, flags)
    }

    ///
    pub fn create_compute_pool(
        &self,
        capacity: usize,
        flags: CommandPoolCreateFlags,
    ) -> CommandPool<B, Compute>
    where
        C: Supports<Compute>
    {
        CommandPool::from_queue(self, capacity, flags)
    }

    ///
    pub fn create_transfer_pool(
        &self,
        capacity: usize,
        flags: CommandPoolCreateFlags,
    ) -> CommandPool<B, Transfer>
    where
        C: Supports<Transfer>
    {
        CommandPool::from_queue(self, capacity, flags)
    }
}
