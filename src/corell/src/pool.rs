use std::ops::{DerefMut};
use CommandPool;
pub use queue::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};

/// General command pool can allocate general command buffers.
pub trait GeneralCommandPool: CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> Self
        where Q: Into<GeneralQueue<Self::Queue>> +
                 DerefMut<Target=Self::Queue>;
}

/// Graphics command pool can allocate graphics command buffers.
pub trait GraphicsCommandPool: CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> Self
        where Q: Into<GraphicsQueue<Self::Queue>> +
                 DerefMut<Target=Self::Queue>;
}

/// Compute command pool can allocate compute command buffers.
pub trait ComputeCommandPool: CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> Self
        where Q: Into<ComputeQueue<Self::Queue>> +
                 DerefMut<Target=Self::Queue>;
}

/// Transfer command pool can allocate transfer command buffers.
pub trait TransferCommandPool: CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> Self
        where Q: Into<TransferQueue<Self::Queue>> +
                 DerefMut<Target=Self::Queue>;
}

/// Subpass command pool can allocate subpass command buffers.
pub trait SubpassCommandPool: CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> Self
        where Q: Into<GraphicsQueue<Self::Queue>> +
                 DerefMut<Target=Self::Queue>;
}
