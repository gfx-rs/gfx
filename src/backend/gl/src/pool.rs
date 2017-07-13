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

use core::{self, pool};
use core::command::{Buffer, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, Encoder};
use core::queue::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use command::{self, RawCommandBuffer, SubpassCommandBuffer};
use {Backend, CommandQueue, FrameBuffer, Resources, Share};
use gl;
use std::rc::Rc;

fn create_fbo_internal(gl: &gl::Gl) -> gl::types::GLuint {
    let mut name = 0 as ::FrameBuffer;
    unsafe {
        gl.GenFramebuffers(1, &mut name);
    }
    info!("\tCreated frame buffer {}", name);
    name
}

pub struct RawCommandPool {
    fbo: FrameBuffer,
    command_buffers: Vec<RawCommandBuffer>,
    next_buffer: usize,
}

impl core::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        self.next_buffer = 0;
        for cb in &mut self.command_buffers {
            cb.reset();
        }
    }

    fn reserve(&mut self, additional: usize) {
        for _ in 0..additional {
            self.command_buffers.push(RawCommandBuffer::new(self.fbo));
        }
    }

    unsafe fn from_queue<'a, Q>(mut queue: Q, capacity: usize) -> Self
    where Q: AsRef<CommandQueue>
    {
        let queue = queue.as_ref();
        let fbo = create_fbo_internal(&queue.share.context);
        let buffers = (0..capacity).map(|_| RawCommandBuffer::new(fbo))
                                   .collect();
        RawCommandPool {
            fbo,
            command_buffers: buffers,
            next_buffer: 0,
        }
    }

    unsafe fn acquire_command_buffer(&mut self) -> &mut RawCommandBuffer {
        let available_buffers = self.command_buffers.len() as isize - self.next_buffer as isize;
        if available_buffers <= 0 {
            self.reserve((-available_buffers) as usize + 1);
        }

        let buffer = &mut self.command_buffers[self.next_buffer];
        self.next_buffer += 1;
        buffer
    }
}

pub struct SubpassCommandPool {
    command_buffers: Vec<SubpassCommandBuffer>,
    next_buffer: usize,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {
    /*
    fn reset(&mut self) {
        self.next_buffer = 0;
    }

    fn reserve(&mut self, additional: usize) {
        for _ in 0..additional {
            self.command_buffers.push(SubpassCommandBuffer::new());
        }
    }

    fn from_queue<'a, Q>(mut _queue: Q, capacity: usize) -> Self
        where Q: Compatible<GraphicsQueue<Backend>> + AsRef<CommandQueue>
    {
        let buffers = (0..capacity).map(|_| SubpassCommandBuffer::new())
                                   .collect();
        SubpassCommandPool {
            command_buffers: buffers,
            next_buffer: 0,
        }
    }

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, SubpassCommandBuffer> {
        let available_buffers = self.command_buffers.len() as isize - self.next_buffer as isize;
        if available_buffers <= 0 {
            self.reserve((-available_buffers) as usize + 1);
        }

        let buffer = &mut self.command_buffers[self.next_buffer];
        self.next_buffer += 1;

        unsafe { Encoder::new(buffer) }
    }
    */
}
