// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Command queue types.
//!
//! There are different types of queues, which can create and submit associated command buffers.

use {pso, Backend, Resources, handle};
use command::{AccessInfo, Submit};
use pool::{GeneralCommandPool, GraphicsCommandPool, ComputeCommandPool,
           TransferCommandPool, SubpassCommandPool};
use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// `QueueFamily` denotes a group of command queues provided by the backend
/// with the same properties/type.
pub trait QueueFamily: 'static {
    /// Return the number of available queues of this family
    // TODO: some backends like d3d12 support infinite software queues (verify)
    fn num_queues(&self) -> u32;
}

/// Submission information for a command queue.
pub struct QueueSubmit<'a, B: Backend + 'a> {
    /// Command buffers to submit.
    pub cmd_buffers: &'a [Submit<B>],
    /// Semaphores to wait being signaled before submission.
    pub wait_semaphores: &'a [(&'a handle::Semaphore<B::Resources>, pso::PipelineStage)],
    /// Semaphores which get signaled after submission.
    pub signal_semaphores: &'a [&'a handle::Semaphore<B::Resources>],
}

/// `CommandQueues` are abstractions to the internal GPU execution engines.
/// Commands are executed on the the device by submitting command buffers to queues.
pub trait CommandQueue<B: Backend> {
    /// Submit command buffers to queue for execution.
    /// `fence` will be signalled after submission and _must_ be unsignalled.
    // TODO: `access` legacy (handle API)
    #[doc(hidden)]
    unsafe fn submit(
        &mut self,
        submit_infos: &[QueueSubmit<B>],
        fence: Option<&handle::Fence<B::Resources>>,
        access: &AccessInfo<B::Resources>,
    );

    ///
    fn wait_idle(&mut self);

    /// Pin everything from this handle manager to live for a frame.
    // TODO: legacy (handle API)
    fn pin_submitted_resources(&mut self, &handle::Manager<B::Resources>);

    /// Cleanup unused resources. This should be called between frames.
    // TODO: legacy (handle API)
    fn cleanup(&mut self);
}

/// Defines queue compatibility regarding functionality.
///
/// Queue A is compatible with queue B if A supports all functionalities from B.
pub trait Compatible<Q> {}

macro_rules! define_queue {
    // Bare queue definitions
    (($queue:ident, $queue_ref:ident, $queue_mut:ident)
        can ()
        derives ()) =>
    (
        ///
        pub struct $queue<B: Backend>(B::CommandQueue);
        ///
        pub struct $queue_ref<'a, B: Backend>(&'a B::CommandQueue)
            where B::CommandQueue: 'a;
        ///
        pub struct $queue_mut<'a, B: Backend>(&'a mut B::CommandQueue)
            where B::CommandQueue: 'a;

        impl<B: Backend> CommandQueue<B> for $queue<B> {
            unsafe fn submit(&mut self, submit_infos: &[QueueSubmit<B>], fence: Option<&handle::Fence<B::Resources>>,
                access: &AccessInfo<B::Resources>) {
                self.0.submit(submit_infos, fence, access)
            }

            fn wait_idle(&mut self) {
                self.0.wait_idle()
            }

            fn pin_submitted_resources(&mut self, handles: &handle::Manager<B::Resources>) {
                self.0.pin_submitted_resources(handles)
            }

            fn cleanup(&mut self) {
                self.0.cleanup()
            }
        }

        impl<'a, B: Backend> CommandQueue<B> for $queue_mut<'a, B> {
            unsafe fn submit(&mut self, submit_infos: &[QueueSubmit<B>], fence: Option<&handle::Fence<B::Resources>>,
                access: &AccessInfo<B::Resources>) {
                self.0.submit(submit_infos, fence, access)
            }

            fn wait_idle(&mut self) {
                self.0.wait_idle()
            }

            fn pin_submitted_resources(&mut self, handles: &handle::Manager<B::Resources>) {
                self.0.pin_submitted_resources(handles)
            }

            fn cleanup(&mut self) {
                self.0.cleanup()
            }
        }

        impl<B: Backend> $queue<B> {
            #[doc(hidden)]
            pub unsafe fn new(queue: B::CommandQueue) -> Self {
                $queue(queue)
            }

            ///
            pub fn as_ref(&self) -> $queue_ref<B> {
                $queue_ref(&self.0)
            }

            ///
            pub fn as_mut(&mut self) -> $queue_mut<B> {
                $queue_mut(&mut self.0)
            }
        }

        impl<'a, B: Backend> $queue_mut<'a, B> {
            ///
            pub fn as_ref(&self) -> $queue_ref<B> {
                $queue_ref(self.0)
            }
        }

        impl<'a, B: Backend> Clone for $queue_ref<'a, B> {
            fn clone(&self) -> Self {
                $queue_ref(self.0.clone())
            }
        }

        // Self compatibility
        impl<B: Backend> Compatible<$queue<B>> for $queue<B> { }
        impl<'a, B: Backend> Compatible<$queue<B>> for &'a $queue<B> { }
        impl<'a, B: Backend> Compatible<$queue<B>> for $queue_ref<'a, B> { }
        impl<'a, B: Backend> Compatible<$queue<B>> for &'a $queue_ref<'a, B> { }
        impl<'a, B: Backend> Compatible<$queue<B>> for $queue_mut<'a, B> { }
        impl<'a, B: Backend> Compatible<$queue<B>> for &'a $queue_mut<'a, B> { }

        impl<B: Backend> AsRef<B::CommandQueue> for $queue<B> {
            fn as_ref(&self) -> &B::CommandQueue {
                &self.0
            }
        }

        impl<'a, B: Backend> AsRef<B::CommandQueue> for $queue_ref<'a, B> {
            fn as_ref(&self) -> &B::CommandQueue {
                self.0
            }
        }

        impl<'a, B: Backend> AsRef<B::CommandQueue> for $queue_mut<'a, B> {
            fn as_ref(&self) -> &B::CommandQueue {
                self.0
            }
        }

        impl<B: Backend> AsMut<B::CommandQueue> for $queue<B> {
            fn as_mut(&mut self) -> &mut B::CommandQueue {
                &mut self.0
            }
        }

        impl<'a, B: Backend> AsMut<B::CommandQueue> for $queue_mut<'a, B> {
            fn as_mut(&mut self) -> &mut B::CommandQueue {
                self.0
            }
        }
    );

    // Impl conversion to other queues
    (($queue:ident, $queue_ref:ident, $queue_mut:ident)
        can ($($submit:ident)*)
        derives ($derive:ident, $derive_ref:ident, $derive_mut:ident
                $($tail_derive:ident, $tail_derive_ref:ident, $tail_derive_mut:ident)*)) =>
    (
        impl<B: Backend> From<$queue<B>> for $derive<B> {
            fn from(queue: $queue<B>) -> Self {
                $derive(queue.0)
            }
        }

        impl<'a, B: Backend> From<$queue_ref<'a, B>> for $derive_ref<'a, B> {
            fn from(queue: $queue_ref<'a, B>) -> Self {
                $derive_ref(queue.0)
            }
        }

        impl<'a, B: Backend> From<$queue_mut<'a, B>> for $derive_ref<'a, B> {
            fn from(queue: $queue_mut<'a, B>) -> Self {
                $derive_ref(queue.0)
            }
        }

        impl<'a, B: Backend> From<$queue_mut<'a, B>> for $derive_mut<'a, B> {
            fn from(queue: $queue_mut<'a, B>) -> Self {
                $derive_mut(queue.0)
            }
        }

        impl<B: Backend> Compatible<$derive<B>> for $queue<B> { }
        impl<'a, B: Backend> Compatible<$derive<B>> for &'a $queue<B> { }
        impl<'a, B: Backend> Compatible<$derive<B>> for $queue_ref<'a, B> { }
        impl<'a, B: Backend> Compatible<$derive<B>> for &'a $queue_ref<'a, B> { }
        impl<'a, B: Backend> Compatible<$derive<B>> for $queue_mut<'a, B> { }
        impl<'a, B: Backend> Compatible<$derive<B>> for &'a $queue_mut<'a, B> { }

        define_queue! {
            ($queue, $queue_ref, $queue_mut)
                can ($($submit)*)
                derives ($($tail_derive, $tail_derive_ref, $tail_derive_mut)*)
        }
    );

    // Impl submits
    (($queue:ident, $queue_ref:ident, $queue_mut:ident)
        can ($submit:ident $($tail_submit:ident)*)
        derives ()) =>
    (
        impl<B: Backend> $queue<B> {
            /// Submit command buffers for execution.
            pub fn $submit(&mut self, submit: &[QueueSubmit<B>], fence: Option<&handle::Fence<B::Resources>>, access: &AccessInfo<B::Resources>) {
                unsafe { self.0.submit(submit, fence, access) }
            }
        }

        impl<'a, B: Backend> $queue_mut<'a, B> {
            /// Submit command buffers for execution.
            pub fn $submit(&mut self, submit: &[QueueSubmit<B>], fence: Option<&handle::Fence<B::Resources>>, access: &AccessInfo<B::Resources>) {
                unsafe { self.0.submit(submit, fence, access) }
            }
        }

        define_queue! {
            ($queue, $queue_ref, $queue_mut)
                can ($($tail_submit)*)
                derives ()
        }
    );
}

define_queue! {
    (GeneralQueue, GeneralQueueRef, GeneralQueueMut)
        can (submit_general submit_graphics submit_compute submit_transfer)
        derives (GraphicsQueue, GraphicsQueueRef, GraphicsQueueMut
                 ComputeQueue, ComputeQueueRef, ComputeQueueMut
                 TransferQueue, TransferQueueRef, TransferQueueMut)
}

define_queue! {
    (GraphicsQueue, GraphicsQueueRef, GraphicsQueueMut)
        can (submit_graphics submit_transfer)
        derives (TransferQueue, TransferQueueRef, TransferQueueMut)
}

define_queue! {
    (ComputeQueue, ComputeQueueRef, ComputeQueueMut)
        can (submit_compute submit_transfer)
        derives (TransferQueue, TransferQueueRef, TransferQueueMut)
}

define_queue! {
    (TransferQueue, TransferQueueRef, TransferQueueMut)
        can (submit_transfer)
        derives ()
}

// Command pool creation implementations

macro_rules! impl_create_pool {
    ($func:ident $pool:ident for) => ();
    ($func:ident $pool:ident for $queue:ident $($tail:ident)*) => (
        impl<B: Backend> $queue<B> {
            /// Create a new command pool with given number of command buffers.
            pub fn $func(&self, capacity: usize) -> B::$pool {
                B::$pool::from_queue(self, capacity)
            }
        }

        impl_create_pool!($func $pool for $($tail)*);
    );
}

impl_create_pool!(create_general_pool GeneralCommandPool for GeneralQueue);
impl_create_pool!(create_graphics_pool GraphicsCommandPool for GeneralQueue GraphicsQueue);
impl_create_pool!(create_compute_pool ComputeCommandPool for GeneralQueue ComputeQueue);
impl_create_pool!(create_transfer_pool TransferCommandPool for GeneralQueue GraphicsQueue ComputeQueue TransferQueue);
impl_create_pool!(create_subpass_pool SubpassCommandPool for GeneralQueue GraphicsQueue);

macro_rules! impl_create_pool_ref {
    ($func:ident $pool:ident for) => ();
    ($func:ident $pool:ident for $queue:ident $($tail:ident)*) => (
        impl<'a, B: Backend> $queue<'a, B> {
            /// Create a new command pool with given number of command buffers.
            pub fn $func(&self, capacity: usize) -> B::$pool {
                B::$pool::from_queue(self, capacity)
            }
        }

        impl_create_pool_ref!($func $pool for $($tail)*);
    );
}

impl_create_pool_ref!(create_general_pool GeneralCommandPool for
    GeneralQueueRef GeneralQueueMut);
impl_create_pool_ref!(create_graphics_pool GraphicsCommandPool for
    GeneralQueueRef GeneralQueueMut
    GraphicsQueueRef GraphicsQueueMut);
impl_create_pool_ref!(create_compute_pool ComputeCommandPool for
    GeneralQueueRef GeneralQueueMut
    ComputeQueueRef ComputeQueueMut);
impl_create_pool_ref!(create_transfer_pool TransferCommandPool for
    GeneralQueueRef GeneralQueueMut
    GraphicsQueueRef GraphicsQueueMut
    ComputeQueueRef ComputeQueueMut
    TransferQueueRef TransferQueueMut);
impl_create_pool_ref!(create_subpass_pool SubpassCommandPool for
    GeneralQueueRef GeneralQueueMut
    GraphicsQueueRef GraphicsQueueMut);
