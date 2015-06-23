#![allow(missing_docs)]
use device::{Capabilities, Device, Resources, SubmitInfo};
use device::command::{CommandBuffer};

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
    fn submit(&mut self, (cb, db, handles): SubmitInfo<Self>) {}
    fn cleanup(&mut self) {}
}
