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

#[allow(missing_copy_implementations)]
pub struct RawCommandPool {
    fbo: n::FrameBuffer,
    limits: command::Limits,
}

impl core::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    unsafe fn from_queue(mut queue: &CommandQueue) -> Self {
        let fbo = create_fbo_internal(&queue.share.context);
        let limits = queue.share.limits.into();
        RawCommandPool {
            fbo,
            limits,
        }
    }

    fn allocate(&mut self, num: usize) -> Vec<RawCommandBuffer> {
        (0..num).map(|_| RawCommandBuffer::new(self.fbo, self.limits)).collect()
    }

    unsafe fn free(&mut self, buffer: Vec<RawCommandBuffer>) {
        // no-op
    }
}

pub struct SubpassCommandPool {
    command_buffers: Vec<SubpassCommandBuffer>,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool { }
