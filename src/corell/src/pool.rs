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

use std::ops::{DerefMut};
use {CommandPool, CommandQueue};
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
