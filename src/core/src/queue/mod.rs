//! Command queues.
//!
//! Queues are the execution paths of the graphical processing units. These process
//! submitted commands buffers.
//!
//! There are different types of queues, which can only handle associated command buffers.
//! Queues are differed by there functionality: graphics, compute and transfer.
//!
//! * `GeneralQueue` supports graphics, compute and transfer.
//! * `GraphicsQueue` supports graphics and transfer.
//! * `ComputeQueue` supports compute and transfer.
//! * `TransferQueue` supports transfer.
//!
pub mod capability;
pub mod submission;

use Backend;
use pool::{GeneralCommandPool, GraphicsCommandPool, ComputeCommandPool,
           TransferCommandPool, RawCommandPool};

pub use self::capability::{Compute, Graphics, General, Transfer, Supports, SupportedBy};
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

/// `CommandQueues` are abstractions to the internal GPU execution engines.
/// Commands are executed on the the device by submitting command buffers to queues.
pub trait CommandQueue<B: Backend> {
    /// Submit command buffers to queue for execution.
    /// `fence` will be signalled after submission and _must_ be unsignalled.
    ///
    /// Unsafe because it's not checked that the queue can process the submitted command buffers.
    /// Trying to submit compute commands to a graphics queue will result in undefined behavior.
    /// Each queue implements safe wrappers according to their supported functionalities!
    unsafe fn submit_raw(&mut self, RawSubmission<B>, Option<&B::Fence>);
}

macro_rules! define_queue {
    () => ();
    // Bare queue definitions
    ($queue:ident $capability:ident $($tail:ident)*) => (
        ///
        pub struct $queue<B: Backend>(B::CommandQueue);

        impl<B: Backend> CommandQueue<B> for $queue<B> {
            unsafe fn submit_raw(&mut self,
                submission: RawSubmission<B>,
                fence: Option<&B::Fence>,
            ) {
                self.0.submit_raw(submission, fence)
            }
        }

        impl<B: Backend> $queue<B> {
            #[doc(hidden)]
            pub unsafe fn new(queue: B::CommandQueue) -> Self {
                $queue(queue)
            }

            ///
            pub fn submit<C>(
                &mut self,
                submission: Submission<B, C>,
                fence: Option<&B::Fence>,
            )
            where
                C: SupportedBy<$capability>
            {
                unsafe {
                    self.submit_raw(submission.as_raw(), fence)
                }
            }
        }

        impl<B: Backend> AsRef<B::CommandQueue> for $queue<B> {
            fn as_ref(&self) -> &B::CommandQueue {
                &self.0
            }
        }

        impl<B: Backend> AsMut<B::CommandQueue> for $queue<B> {
            fn as_mut(&mut self) -> &mut B::CommandQueue {
                &mut self.0
            }
        }

        define_queue! { $($tail)* }
    );
}

define_queue! {
    GeneralQueue General
    GraphicsQueue Graphics
    ComputeQueue Compute
    TransferQueue Transfer
}

// Command pool creation implementations
macro_rules! impl_create_pool {
    ($func:ident $pool:ident for) => ();
    ($func:ident $pool:ident for $queue:ident $($tail:ident)*) => (
        impl<B: Backend> $queue<B> {
            /// Create a new command pool with given number of command buffers.
            pub fn $func(&self, capacity: usize) -> $pool<B> {
                $pool(unsafe { B::RawCommandPool::from_queue(self, capacity) })
            }
        }

        impl_create_pool!($func $pool for $($tail)*);
    );
}

impl_create_pool!(create_general_pool GeneralCommandPool for GeneralQueue);
impl_create_pool!(create_graphics_pool GraphicsCommandPool for GeneralQueue GraphicsQueue);
impl_create_pool!(create_compute_pool ComputeCommandPool for GeneralQueue ComputeQueue);
impl_create_pool!(create_transfer_pool TransferCommandPool for GeneralQueue GraphicsQueue ComputeQueue TransferQueue);
// impl_create_pool!(create_subpass_pool SubpassCommandPool for GeneralQueue GraphicsQueue);
