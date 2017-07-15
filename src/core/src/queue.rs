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
           TransferCommandPool, RawCommandPool};
use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

///
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    ///
    pub fn supports_graphics(&self) -> bool {
        *self == QueueType::General || *self == QueueType::Graphics
    }

    ///
    pub fn supports_compute(&self) -> bool {
        *self == QueueType::General || *self == QueueType::Compute
    }

    ///
    pub fn supports_transfer(&self) -> bool { true }
}

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

    /// Pin everything from this handle manager to live for a frame.
    // TODO: legacy (handle API)
    fn pin_submitted_resources(&mut self, &handle::Manager<B::Resources>);

    /// Cleanup unused resources. This should be called between frames.
    // TODO: legacy (handle API)
    fn cleanup(&mut self);
}

macro_rules! define_queue {
    // Bare queue definitions
    ($queue:ident can ()) => (
        ///
        pub struct $queue<B: Backend>(B::CommandQueue);

        impl<B: Backend> CommandQueue<B> for $queue<B> {
            unsafe fn submit(&mut self, submit_infos: &[QueueSubmit<B>], fence: Option<&handle::Fence<B::Resources>>,
                access: &AccessInfo<B::Resources>) {
                self.0.submit(submit_infos, fence, access)
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
    );

    // Impl submits
    ($queue:ident can ($submit:ident $($tail_submit:ident)*)) => (
        impl<B: Backend> $queue<B> {
            /// Submit command buffers for execution.
            pub fn $submit(&mut self, submit: &[QueueSubmit<B>], fence: Option<&handle::Fence<B::Resources>>, access: &AccessInfo<B::Resources>) {
                unsafe { self.0.submit(submit, fence, access) }
            }
        }

        define_queue! {
            $queue can ($($tail_submit)*)
        }
    );
}

define_queue! {
    GeneralQueue can (submit_general submit_graphics submit_compute submit_transfer)
}

define_queue! {
    GraphicsQueue can (submit_graphics submit_transfer)
}

define_queue! {
    ComputeQueue can (submit_compute submit_transfer)
}

define_queue! {
    TransferQueue can (submit_transfer)
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
