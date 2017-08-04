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

//! Queue submission.
//!
//! // TODO

use {handle, pso, Backend};
use command::{Submit, GeneralCommandBuffer, GraphicsCommandBuffer,
              ComputeCommandBuffer, TransferCommandBuffer};
use super::capability::{General, Graphics, Compute, Transfer, Supports, Upper};
use std::marker::PhantomData;
use smallvec::SmallVec;

use super::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};

/// Raw submission information for a command queue.
pub struct RawSubmission<'a, B: Backend + 'a> {
    /// Command buffers to submit.
    pub cmd_buffers: &'a [B::SubmitInfo],
    /// Semaphores to wait being signalled before submission.
    pub wait_semaphores: &'a [(&'a handle::Semaphore<B>, pso::PipelineStage)],
    /// Semaphores which get signalled after submission.
    pub signal_semaphores: &'a [&'a handle::Semaphore<B>],
}

/// Submission information for a command queue.
pub struct Submission<'a, B: Backend, C> {
    cmd_buffers: SmallVec<[B::SubmitInfo; 16]>,
    wait_semaphores: SmallVec<[(&'a handle::Semaphore<B>, pso::PipelineStage); 16]>,
    signal_semaphores: SmallVec<[&'a handle::Semaphore<B>; 16]>,
    marker: PhantomData<C>,
}

impl<'a, B: Backend> Submission<'a, B, Transfer> {
    /// Create a new empty (transfer) submission.
    ///
    /// Transfer is the minimum supported capability by all queues.
    pub fn new() -> Submission<'a, B, Transfer> {
        Submission {
            cmd_buffers: SmallVec::new(),
            wait_semaphores: SmallVec::new(),
            signal_semaphores: SmallVec::new(),
            marker: PhantomData,
        }
    }
}

impl<'a, B, C> Submission<'a, B, C>
where
    B: Backend
{
    /// Set semaphores which will waited on to be signalled before the submission will be executed.
    pub fn wait_on(mut self, semaphores: &[(&'a handle::Semaphore<B>, pso::PipelineStage)]) -> Self {
        self.wait_semaphores.extend_from_slice(semaphores);
        self
    }

    /// Set semaphores which will be signalled once this submission has finished executing.
    pub fn signal(mut self, semaphores: &[&'a handle::Semaphore<B>]) -> Self {
        self.signal_semaphores.extend_from_slice(semaphores);
        self
    }

    /// Convert strong-typed submission object into untyped equivalent.
    pub(super) fn as_raw(&self) -> RawSubmission<B> {
        RawSubmission {
            cmd_buffers: &self.cmd_buffers,
            wait_semaphores: &self.wait_semaphores,
            signal_semaphores: &self.signal_semaphores,
        }
    }

    /// Append a new list of finished command buffers to this submission.
    ///
    /// All submits for this call must be of the same capability.
    /// Submission will be automatically promoted to to the minimum required capability
    /// to hold all passed submits.
    pub fn submit<S>(mut self, submits: &[Submit<B, S>]) -> Submission<'a, B, <(C, S) as Upper>::Result>
    where
        (C, S): Upper
    {
        self.cmd_buffers.extend(submits.iter().map(|submit| unsafe { submit.get_info().clone() }));
        Submission {
            cmd_buffers: self.cmd_buffers,
            wait_semaphores: self.wait_semaphores,
            signal_semaphores: self.signal_semaphores,
            marker: PhantomData,
        }
    }

    /// Promote a submission to a higher capability type.
    ///
    /// Submission promotion is only necessary for shoving multiple submissions
    /// of different capabilities into one submit call.
    pub fn promote<P>(self) -> Submission<'a, B, P>
    where
        P: Supports<C>
    {
        Submission {
            cmd_buffers: self.cmd_buffers,
            wait_semaphores: self.wait_semaphores,
            signal_semaphores: self.signal_semaphores,
            marker: PhantomData,
        }
    }
}
