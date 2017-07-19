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

use {Backend, CommandQueue};
use command::RawCommandBuffer;
use core::pool;

pub struct RawCommandPool {
}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, additional: usize) {
        unimplemented!()
    }

    unsafe fn from_queue<Q>(mut queue: Q, capacity: usize) -> RawCommandPool
    where Q: AsRef<CommandQueue>
    {
        unimplemented!()
    }

    unsafe fn acquire_command_buffer(&mut self) -> &mut RawCommandBuffer {
        unimplemented!()
    }
}

pub struct SubpassCommandPool;
impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {

}