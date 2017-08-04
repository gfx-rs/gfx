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

use Backend;
use queue::capability::{Capability, Transfer};
use super::{RawCommandBuffer, Submit};

/// Command buffer with transfer functionality.
pub struct TransferCommandBuffer<'a, B: Backend>(pub(crate) &'a mut B::RawCommandBuffer)
where B::RawCommandBuffer: 'a;

impl<'a, B: Backend> Capability for TransferCommandBuffer<'a, B> {
    type Capability = Transfer;
}

impl<'a, B: Backend> TransferCommandBuffer<'a, B> {
    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(mut self) -> Submit<B, Transfer> {
        Submit::new(self.0.finish())
    }
}
