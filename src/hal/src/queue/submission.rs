//! A `Submission` is simply a collection of data bundled up and ready
//! to be submitted to a command queue.

use {pso, Backend};
use command::{Submittable, Primary};
use super::capability::{Transfer, Supports, Upper};
use std::borrow::{Borrow, Cow};
use std::ops::Deref;
use std::marker::PhantomData;
use smallvec::SmallVec;

/// Raw submission information for a command queue.
pub struct RawSubmission<'a, B: Backend + 'a, IC>
where
    IC: IntoIterator,
    IC::Item: Borrow<B::CommandBuffer>,
{
    /// Command buffers to submit.
    pub cmd_buffers: IC,
    /// Semaphores to wait being signalled before submission.
    pub wait_semaphores: &'a [(&'a B::Semaphore, pso::PipelineStage)],
    /// Semaphores which get signalled after submission.
    pub signal_semaphores: &'a [&'a B::Semaphore],
}

/// Submission information for a command queue, generic over a particular
/// backend and a particular queue type.
pub struct Submission<'a, B: Backend, C> {
    cmd_buffers: SmallVec<[Cow<'a, B::CommandBuffer>; 16]>,
    wait_semaphores: SmallVec<[(&'a B::Semaphore, pso::PipelineStage); 16]>,
    signal_semaphores: SmallVec<[&'a B::Semaphore; 16]>,
    marker: PhantomData<C>,
}

impl<'a, B: Backend> Submission<'a, B, Transfer> {
    /// Create a new empty (transfer) submission.
    ///
    /// Transfer is the minimum supported capability by all queues.
    pub fn new() -> Self {
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
    /// Add to semaphores which will waited on to be signalled before the submission will be executed.
    pub fn wait_on<I>(mut self, semaphores: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Semaphore, pso::PipelineStage)>,
    {
        self.wait_semaphores.extend(semaphores.into_iter().map(|semaphore| *semaphore.borrow()));
        self
    }

    /// Add to semaphores which will be signalled once this submission has finished executing.
    pub fn signal<I>(mut self, semaphores: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<&'a B::Semaphore>,
    {
        self.signal_semaphores.extend(semaphores.into_iter().map(|semaphore| *semaphore.borrow()));
        self
    }

    /// Convert strong-typed submission object into untyped equivalent.
    pub(super) fn to_raw(&self) -> RawSubmission<B, Vec<&B::CommandBuffer>> {
        RawSubmission {
            cmd_buffers: self.cmd_buffers.iter().map(|b| b.deref().borrow()).collect::<Vec<_>>(),
            wait_semaphores: &self.wait_semaphores,
            signal_semaphores: &self.signal_semaphores,
        }
    }

    /// Append a new list of finished command buffers to this submission.
    ///
    /// All submits for this call must be of the same type.
    /// Submission will be automatically promoted to to the minimum required capability
    /// to hold all passed submits.
    pub fn submit<I, K>(mut self, submits: I) -> Submission<'a, B, <(C, K) as Upper>::Result>
    where
        I: IntoIterator,
        I::Item: Submittable<'a, B, K, Primary>,
        (C, K): Upper
    {
        self.cmd_buffers.extend(submits.into_iter().map(
            |s| { unsafe { s.into_buffer() } }
        ));
        Submission {
            cmd_buffers: self.cmd_buffers,
            wait_semaphores: self.wait_semaphores,
            signal_semaphores: self.signal_semaphores,
            marker: PhantomData,
        }
    }

    /// Promote a submission to a higher (more general) capability type. For example,
    /// this can turn a `Compute` submission into a `GraphicsOrCompute` submission.
    ///
    /// Submission promotion is only necessary for combining multiple submissions
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
