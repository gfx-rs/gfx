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
use Platform;

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

    pub fn clear(&self, r: f32, g: f32, b: f32) {
        let (_, _, _) = (r, g, b);
        self.cast(unimplemented!());
    }
}

pub struct Server<P> {
    no_send: marker::NoSend,
    no_share: marker::NoShare,
    stream: DuplexStream<Reply, Request>,
    platform: P,
}

impl<Api, P: Platform<Api>> Server<P> {
    /// Update the platform. The client must manually update this on the main
    /// thread.
    pub fn update(&self) {
        // Get updates from the renderer and pass on results
        'recv: loop {
            match self.stream.try_recv() {
                Ok(server::Cast(_)) => {},
                Ok(server::Call(_)) => self.stream.send(unimplemented!()),
                Err(comm::Empty) => break 'recv,
                Err(comm::Disconnected) => fail!("Render task has closed."),
            }
        }

        self.platform.swap_buffers();
    }
}

#[deriving(Show)]
pub enum InitError {}

pub fn init<Api, P: Platform<Api>>(platform: P, _: super::Options)
        -> Result<(Client, Server<P>), InitError> {
    let (client_stream, server_stream) = comm::duplex();

    let client = Client {
        stream: client_stream,
    };
    let server = Server {
        no_send: marker::NoSend,
        no_share: marker::NoShare,
        stream: server_stream,
        platform: platform,
    };

    Ok((client, server))
}
