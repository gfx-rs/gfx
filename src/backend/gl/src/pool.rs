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
use command::{self, RawCommandBuffer, SubpassCommandBuffer};
use native as n;
use {Backend, CommandQueue, Share};
use gl;
use std::rc::Rc;

fn create_fbo_internal(gl: &gl::Gl) -> gl::types::GLuint {
    let mut name = 0 as n::FrameBuffer;
    unsafe {
        gl.GenFramebuffers(1, &mut name);
    }
    info!("\tCreated frame buffer {}", name);
    name
}

pub struct RawCommandPool {
    fbo: n::FrameBuffer,
    limits: command::Limits,
    command_buffers: Vec<RawCommandBuffer>,
}

impl core::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        for cb in &mut self.command_buffers {
            cb.reset();
        }
    }

    fn reserve(&mut self, additional: usize) {
        for _ in 0..additional {
            self.command_buffers.push(RawCommandBuffer::new(self.fbo, self.limits));
        }
    }

    unsafe fn from_queue(mut queue: &CommandQueue, capacity: usize) -> Self {
        let fbo = create_fbo_internal(&queue.share.context);
        let limits = queue.share.limits.into();
        let buffers = (0..capacity).map(|_| RawCommandBuffer::new(fbo, limits)).collect();
        RawCommandPool {
            fbo,
            limits,
            command_buffers: buffers,
        }
    }

    unsafe fn acquire_command_buffer(&mut self) -> RawCommandBuffer {
        // TODO: rewrite _without_ usage of 'unwrap'
        if self.command_buffers.len() <= 0 {
            self.reserve(1);
        }

        self.command_buffers.pop().unwrap()
    }

    unsafe fn return_command_buffer(&mut self, buffer: RawCommandBuffer) {
        self.command_buffers.push(buffer)
    }
}

pub struct SubpassCommandPool {
    command_buffers: Vec<SubpassCommandBuffer>,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool { }
