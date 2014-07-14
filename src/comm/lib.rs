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

#![crate_name = "comm"]
#![comment = "Internal concurrency constructs for gfx-rs."]
#![license = "ASL2"]
#![crate_type = "lib"]

#[deriving(PartialEq, Eq)]
struct RequestClose;

pub struct ShouldClose(Sender<()>, Receiver<RequestClose>);
pub struct Close(Sender<RequestClose>, Receiver<()>);

impl ShouldClose {
    pub fn check(&self) -> bool {
        let ShouldClose(_, ref rx) = *self;
        rx.try_recv() == Ok(RequestClose)
    }
}

impl Close {
    pub fn now(&self) {
        let Close(ref tx, ref rx) = *self;
        let _ = tx.send_opt(RequestClose);
        let _ = rx.recv_opt(); // block until the channel disconnects
    }
}

pub fn close_stream() -> (Close, ShouldClose) {
    let (req_tx, req_rx) = channel();
    let (ack_tx, ack_rx) = channel();
    (Close(req_tx, ack_rx), ShouldClose(ack_tx, req_rx))
}
