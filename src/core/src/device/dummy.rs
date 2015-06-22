#![allow(missing_docs)]
use device::{Device, Resources, Capabilities, SubmitInfo};
use device::command::{GenericCommandBuffer};

pub struct DummyDevice {
    capabilities: Capabilities
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum DummyResources{}

pub type Buffer         = u32;
pub type ArrayBuffer    = u32;
pub type Shader         = u32;
pub type Program        = u32;
pub type FrameBuffer    = u32;
pub type Surface        = u32;
pub type Sampler        = u32;
pub type Texture        = u32;

impl Resources for DummyResources {
    type Buffer         = Buffer;
    type ArrayBuffer    = ArrayBuffer;
    type Shader         = Shader;
    type Program        = Program;
    type FrameBuffer    = FrameBuffer;
    type Surface        = Surface;
    type Texture        = Texture;
    type Sampler        = Sampler;
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
    type CommandBuffer = GenericCommandBuffer<Self::Resources>;

    fn get_capabilities<'a>(&'a self) -> &'a Capabilities {
        &self.capabilities
    }
    fn reset_state(&mut self) {}
    fn submit(&mut self, (cb, db, handles): SubmitInfo<Self>) {}
    fn cleanup(&mut self) {}
}
