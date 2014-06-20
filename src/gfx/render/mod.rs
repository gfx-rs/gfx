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

use server;
use device;

pub enum Call {}
pub enum Cast {
    Clear(f32, f32, f32), // TODO: use color-rs?
    Finish,
}
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
        self.cast(Clear(r, g, b));
    }

    pub fn finish(&self) {
        self.cast(Finish);
    }
}

/// Start a render server using the provided device client
pub fn start(options: (), device: device::Client) -> Client {
    let (render_stream, task_stream) = comm::duplex::<Request, Reply>();
    spawn(proc() {
        'render: loop {
            'recv: loop {
                match task_stream.try_recv() {
                    Err(comm::Disconnected) | Ok(server::Cast(Finish)) => {
                        break 'render; // terminate the rendering task
                                       // TODO: device.finish()?
                    },
                    Ok(server::Cast(Clear(r, g, b))) => {
                        device.clear(r, g, b);
                    },
                    Ok(server::Call(_)) => {
                        task_stream.send(unimplemented!());
                    },
                    Err(comm::Empty)  => {
                        break 'recv; // finished all the pending rendering messages
                    },
                }
            }
        }
    });
    Client {
        stream: render_stream,
    }
}
