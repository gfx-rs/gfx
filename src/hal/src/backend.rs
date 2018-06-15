//! Functionality only required for backend implementations.

use Backend;
use queue::{QueueFamily, Queues};

use std::collections::HashMap;
use std::hash::BuildHasherDefault;

use fxhash::FxHasher;


/// Bare-metal queue group.
///
/// Denotes all queues created from one queue family.
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
