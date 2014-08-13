// Copyright 2014 The Gfx-rs Developers.
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

//! High-level, platform independent, bindless rendering API.

#![crate_name = "render"]
#![comment = "A platform independent renderer for gfx-rs."]
#![license = "ASL2"]
#![crate_type = "lib"]
#![deny(missing_doc)]
#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate device;

use std::cell::Cell;
use std::fmt::Show;
use std::mem;
use std::vec::MoveItems;

use backend = device::back;
use device::shade::{CreateShaderError, Vertex, Fragment, ShaderSource,
    UniformValue};
use device::target::{ClearData, Target, TargetColor, TargetDepth, TargetStencil};
use shade::{ProgramShell, ShaderParam};
use resource::{Loaded, Pending};

pub type SamplerHandle = (backend::Sampler, device::tex::SamplerInfo);

/// Used for sending/receiving handles to/from the device. Not meant for users.
#[experimental]
pub type Token = resource::Handle;
/// Program handle
#[deriving(Clone, PartialEq, Show)]
pub struct ProgramHandle(Token);
/// Shader handle
#[deriving(Clone, PartialEq, Show)]
pub struct ShaderHandle(Token);

/// Frontend
mod front;
/// Meshes
pub mod mesh;
/// Resources
pub mod resource;
/// Shaders
pub mod shade;
/// Draw state
pub mod state;
/// Render targets
pub mod target;

/// Graphics state
struct State {
    frame: target::Frame,
}

/// An error that can happen when sending commands to the device. Any attempt to use the handles
/// returned here will fail.
#[deriving(Clone, Show)]
pub enum DeviceError {
    /// Error creating a new buffer.
    ErrorNewBuffer(backend::Buffer),
    /// Error creating a new array buffer.
    ErrorNewArrayBuffer,
    /// Error creating a new shader.
    ErrorNewShader(ShaderHandle, CreateShaderError),
    /// Error creating a new program.
    ErrorNewProgram(ProgramHandle),
    /// Error creating a new framebuffer.
    ErrorNewFrameBuffer,
}

struct DeviceSender {
    chan: Sender<device::Request<Token>>,
    alive: Cell<bool>,
}

impl DeviceSender {
    fn send(&self, r: device::Request<Token>) {
        self.chan.send_opt(r).map_err(|_| self.alive.set(false));
    }

    fn is_alive(&self) -> bool {
        self.alive.get()
    }
}

/// A renderer. Methods on this get translated into commands for the device.
pub struct Renderer {
    device_tx: DeviceSender,
    swap_ack: Receiver<device::Ack>,
    should_close: bool,
    /// the shared VAO and FBO
    common_array_buffer: Token,
    common_frame_buffer: Token,
    /// the default FBO for drawing
    default_frame_buffer: backend::FrameBuffer,
    /// current state
    state: State,
}

type TempProgramResources = (
    Vec<Option<UniformValue>>,
    Vec<Option<backend::Buffer>>,
    Vec<Option<shade::TextureParam>>
);

impl Renderer {
    /// Create a new `Renderer` using given channels for communicating with the device. Generally,
    /// you want to use `gfx::start` instead.
    pub fn new(device_tx: Sender<device::Request<Token>>, device_rx: Receiver<device::Reply<Token>>,
            swap_rx: Receiver<device::Ack>) -> Renderer {
        // Request the creation of the common array buffer and frame buffer
        let mut res = resource::Cache::new();
        let c_vao = res.array_buffers.add(Pending);
        let c_fbo = res.frame_buffers.add(Pending);
        device_tx.send(device::Call(c_vao, device::CreateArrayBuffer));
        device_tx.send(device::Call(c_fbo, device::CreateFrameBuffer));
        // Return
        Renderer {
            device_tx: DeviceSender {
                chan: device_tx,
                alive: Cell::new(true),
            },
            swap_ack: swap_rx,
            should_close: false,
            common_array_buffer: c_vao,
            common_frame_buffer: c_fbo,
            default_frame_buffer: 0,
            state: State {
                frame: target::Frame::new(0, 0),
            },
        }
    }

    /// Ask the device to do something for us
    fn cast(&self, msg: device::CastRequest) {
        self.device_tx.send(device::Cast(msg));
    }

    /// Whether rendering should stop completely.
    pub fn should_finish(&self) -> bool {
        self.should_close || !self.device_tx.is_alive()
    }

    /// Finish rendering a frame. Waits for a frame to be finished drawing, as specified by the
    /// queue size passed to `gfx::start`.
    pub fn end_frame(&mut self) {
        self.device_tx.send(device::SwapBuffers);
        //wait for acknowlegement
        self.swap_ack.recv_opt().map_err(|_| {
            self.should_close = true; // the channel has disconnected, so it is time to close
        });
    }
}
