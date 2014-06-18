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

/// Start a render server using the provided device client
pub fn start(options: (), device: device::Client) -> Client {
    let (render_stream, task_stream) = comm::duplex::<Request, Reply>();
    spawn(proc() {
        loop {
            // TODO
            let _ = task_stream.recv();
            task_stream.send(unimplemented!());

            // TODO
            let _ = device;
        }
    });
    Client {
        stream: render_stream,
    }
}
