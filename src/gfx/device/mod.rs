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

use std::comm;
use std::comm::DuplexStream;
use std::kinds::marker;

use server;

pub enum Call {}
pub enum Cast {}
pub type Request = server::Request<Call, Cast>;

pub enum Reply {}

pub struct Client {
    stream: DuplexStream<Request, Reply>,
}

impl Client {
    fn call(&self, msg: Call) -> Reply {
        self.stream.send(server::Call(msg));
        self.stream.recv()
    }

    fn cast(&self, msg: Cast) {
        self.stream.send(server::Cast(msg));
    }

    // TODO: public functions
}

pub struct Server {
    no_send: marker::NoSend,
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
}

impl Server {
    /// Update the platform.
    pub fn update(&self) {
        // Poll events
        // Get updates from the renderer and pass on results
        let _ = self.stream.recv();
        self.stream.send(unimplemented!());
    }

    // TODO: command register methods
}

#[deriving(Show)]
pub enum InitError {}

pub fn init(options: super::Options) -> Result<(Client, Server), InitError> {
    // TODO: Platform-specific initialization (GLFW / SDL2, OpenGL)

    let (device_stream, platform_stream) = comm::duplex();

    let device = Client {
        stream: device_stream,
    };
    let platform = Server {
        no_send: marker::NoSend,
        no_share: marker::NoShare,
        stream: platform_stream,
    };

    Ok((device, platform))
}
