// Copyright 2015 The Gfx-rs Developers.
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
#![allow(missing_docs)]
use device::{Capabilities, Device, Resources, SubmitInfo};
use device::command::{CommandBuffer};

///Dummy device which does minimal work, just to allow testing gfx-rs apps for
///compilation.
pub struct DummyDevice {
    capabilities: Capabilities
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum DummyResources{}

impl Resources for DummyResources {
    type Buffer         = ();
    type ArrayBuffer    = ();
    type Shader         = ();
    type Program        = ();
    type FrameBuffer    = ();
    type Surface        = ();
    type Texture        = ();
    type Sampler        = ();
    type Fence          = ();
}

impl DummyDevice {
    fn new(capabilities: Capabilities) -> DummyDevice {
        DummyDevice {
            capabilities: capabilities
        }
    }
}

impl Device for DummyDevice {
    type Resources = DummyResources;
    type CommandBuffer = CommandBuffer<Self::Resources>;

    fn get_capabilities<'a>(&'a self) -> &'a Capabilities {
        &self.capabilities
    }
    fn reset_state(&mut self) {}
    fn submit(&mut self, (_cb, _db, _handles): SubmitInfo<Self>) {}
    fn cleanup(&mut self) {}
}
