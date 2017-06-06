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

use std::borrow::{Borrow, BorrowMut};
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use {Backend, CommandQueue, QueueSubmit, Resources};
use command::Submit;

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

        impl<'a, B: Backend> Clone for $queue_ref<'a, B> {
            fn clone(&self) -> Self {
                $queue_ref(self.0.clone())
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

        impl<'a, B: Backend> From<$queue_mut<'a, B>> for $derive_mut<'a, B> {
            fn from(queue: $queue_mut<'a, B>) -> Self {
                $derive_mut(queue.0)
            }
        }

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
            pub fn $submit(&mut self, submit: &[QueueSubmit<B>], fence: Option<&mut <B::Resources as Resources>::Fence>) {
                unsafe { self.0.submit(submit, fence) }
            } 
        }

        impl<'a, B: Backend> $queue_mut<'a, B> {
            /// Submit command buffers for execution.
            pub fn $submit(&mut self, submit: &[QueueSubmit<B>], fence: Option<&mut <B::Resources as Resources>::Fence>) {
                unsafe { self.0.submit(submit, fence) }
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
