//! Functionality only required for backend implementations.

use crate::buffer::Offset;
use crate::queue::{QueueFamily, Queues};
use crate::Backend;

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::ops::{Bound, Range, RangeBounds};

use fxhash::FxHasher;

/// Bare-metal queue group.
///
/// Denotes all queues created from one queue family.
#[derive(Debug)]
pub struct RawQueueGroup<B: Backend> {
    pub family: B::QueueFamily,
    pub queues: Vec<B::CommandQueue>,
}

impl<B: Backend> RawQueueGroup<B> {
    /// Create a new, empty queue group for a queue family.
    pub fn new(family: B::QueueFamily) -> Self {
        RawQueueGroup {
            family,
            queues: Vec::new(),
        }
    }

    /// Add a command queue to the group.
    ///
    /// The queue needs to be created from this queue family.
    ///
    /// # Panics
    ///
    /// Panics if more command queues are added than exposed by the queue family.
    pub fn add_queue(&mut self, queue: B::CommandQueue) {
        assert!(self.queues.len() < self.family.max_queues());
        self.queues.push(queue);
    }
}

impl<B: Backend> Queues<B> {
    /// Create a new collection of queues.
    pub fn new(queues: Vec<RawQueueGroup<B>>) -> Self {
        Queues(queues)
    }
}

/// Fast hash map used internally.
pub type FastHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// Helper trait that adds methods to `RangeBounds` for buffer ranges
pub trait Bounds {
    fn to_range(&self, end: Offset) -> Range<Offset>;
}

impl<B: RangeBounds<Offset>> Bounds for B {
    fn to_range(&self, end: Offset) -> Range<Offset> {
        Range {
            start: match self.start_bound() {
                Bound::Included(&v) => v,
                Bound::Excluded(&v) => v + 1,
                Bound::Unbounded => 0,
            },
            end: match self.end_bound() {
                Bound::Included(&v) => v + 1,
                Bound::Excluded(&v) => v,
                Bound::Unbounded => end,
            },
        }
    }
}
