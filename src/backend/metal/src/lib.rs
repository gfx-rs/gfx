// Copyright 2016 The Gfx-rs Developers.
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

#[macro_use]
extern crate log;
extern crate objc;
extern crate cocoa;
extern crate gfx_core;
extern crate metal;

use cocoa::base::{selector, class};
use cocoa::foundation::{NSUInteger};

use metal::*;

use gfx_core::format::Format;
use gfx_core::factory::Usage;
use gfx_core::{handle, tex};

use std::cell::RefCell;
use std::sync::Arc;
use std::mem;

mod factory;
mod command;
mod map;

pub use self::command::CommandBuffer;
pub use self::factory::Factory;
pub use self::map::*;

/// Internal struct of shared data between the device and its factories.
#[doc(hidden)]
pub struct Share {
    capabilities: gfx_core::Capabilities,
    handles: RefCell<handle::Manager<Resources>>,
}

pub mod native {
    use metal::*;

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub struct Buffer(pub MTLBuffer);
    unsafe impl Send for Buffer {}
    unsafe impl Sync for Buffer {}

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub struct Texture(pub MTLTexture);
    unsafe impl Send for Texture {}
    unsafe impl Sync for Texture {}

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub struct Rtv(pub MTLTexture);
    unsafe impl Send for Rtv {}
    unsafe impl Sync for Rtv {}

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub struct Dsv(pub MTLTexture);
    unsafe impl Send for Dsv {}
    unsafe impl Sync for Dsv {}

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub struct Srv(pub MTLTexture);
    unsafe impl Send for Srv {}
    unsafe impl Sync for Srv {}
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct InputLayout(pub MTLVertexDescriptor);
unsafe impl Send for InputLayout {}
unsafe impl Sync for InputLayout {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Shader {
    func: MTLFunction
}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    vs: MTLFunction,
    ps: MTLFunction
}
unsafe impl Send for Program {}
unsafe impl Sync for Program {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Pipeline {
    pipeline: MTLRenderPipelineState
}
unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Buffer(native::Buffer, Usage);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Texture(native::Texture, Usage);

pub struct Device {
    device: MTLDevice,
    feature_set: MTLFeatureSet,
    share: Arc<Share>,
    frame_handles: handle::Manager<Resources>,
    max_resource_count: Option<usize>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl gfx_core::Resources for Resources {
    type Buffer              = Buffer;
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = Pipeline;
    type Texture             = Texture;
    type RenderTargetView    = native::Rtv;
    type DepthStencilView    = native::Dsv;
    type ShaderResourceView  = native::Srv;
    type UnorderedAccessView = ();
    type Sampler             = ();
    type Fence               = ();
}

pub type ShaderModel = u16;

impl Device {
    pub fn get_shader_model(&self) -> ShaderModel {
        use metal::MTLFeatureSet::*;

        match self.feature_set {
            iOS_GPUFamily1_v1 |
            iOS_GPUFamily1_v2 => 10,
            iOS_GPUFamily2_v1 |
            iOS_GPUFamily2_v2 |
            iOS_GPUFamily3_v1 |
            OSX_GPUFamily1_v1 => 11,
        }
    }
}

impl gfx_core::Device for Device {
    type Resources = Resources;
    type CommandBuffer = command::CommandBuffer;

    fn get_capabilities(&self) -> &gfx_core::Capabilities {
        &self.share.capabilities
    }

    fn pin_submitted_resources(&mut self, man: &handle::Manager<Resources>) {
        self.frame_handles.extend(man);
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn submit(&mut self, cb: &mut Self::CommandBuffer) {

    }

    fn cleanup(&mut self) {
        use gfx_core::handle::Producer;
        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut (),
            |_, v| unsafe { /*v.0.release();*/ }, //buffer
            |_, s| unsafe { //shader
                /*(*s.object).Release();
                (*s.reflection).Release();*/
            },
            |_, p| unsafe {
                if !p.vs.is_null() { p.vs.release(); }
                if !p.ps.is_null() { p.ps.release(); }
            }, //program
            |_, v| unsafe { //PSO
                /*type Child = *mut winapi::ID3D11DeviceChild;
                (*v.layout).Release();
                (*(v.rasterizer as Child)).Release();
                (*(v.depth_stencil as Child)).Release();
                (*(v.blend as Child)).Release();*/
            },
            |_, v| unsafe { (v.0).0.release(); },  //texture
            |_, v| unsafe { /*(*v.0).Release();*/ }, //SRV
            |_, _| {}, //UAV
            |_, v| unsafe { /*(*v.0).Release();*/ }, //RTV
            |_, v| unsafe { /*(*v.0).Release();*/ }, //DSV
            |_, v| unsafe { /*v.sampler.release();*/ }, //sampler
            |_, _| {}, //fence
        );
    }
}

pub fn create(format: gfx_core::format::Format, width: u32, height: u32)
              -> Result<(Device, Factory, handle::RawRenderTargetView<Resources>, *mut MTLTexture), ()> {
    use gfx_core::handle::Producer;

    let share = Share {
        capabilities: gfx_core::Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: true,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
        },
        handles: RefCell::new(handle::Manager::new()),
    };

    let mtl_device = create_system_default_device();

    let get_feature_set = |device: MTLDevice| -> Option<MTLFeatureSet> {
        use metal::MTLFeatureSet::*;

        let feature_sets = vec![OSX_GPUFamily1_v1, iOS_GPUFamily3_v1,
                                iOS_GPUFamily2_v2, iOS_GPUFamily2_v1,
                                iOS_GPUFamily1_v2, iOS_GPUFamily1_v1];

        for feature in feature_sets.into_iter() {
            if mtl_device.supports_feature_set(feature) {
                return Some(feature);
            }
        }

        return None;
    };

    let raw_tex = Texture(native::Texture(MTLTexture::nil()), Usage::GpuOnly);
    let raw_addr: *mut MTLTexture = unsafe { mem::transmute(&(raw_tex.0).0) };

    let color_tex = share.handles.borrow_mut().make_texture(raw_tex, tex::Descriptor {
        kind: tex::Kind::D2(width as tex::Size, height as tex::Size, tex::AaMode::Single),
        levels: 1,
        format: format.0,
        bind: gfx_core::factory::RENDER_TARGET,
        usage: raw_tex.1,
    });

    let device = Device {
        device: mtl_device,
        feature_set: get_feature_set(mtl_device).unwrap(),
        share: Arc::new(share),
        frame_handles: handle::Manager::new(),
        max_resource_count: None,
    };
    let mut factory = Factory::new(mtl_device, device.share.clone());

    let color_target = {
        use gfx_core::Factory;

        let desc = tex::RenderDesc {
            channel: format.1,
            level: 0,
            layer: None,
        };

        factory.view_texture_as_render_target_raw(&color_tex, desc).unwrap()
    };


    Ok((device, factory, color_target, raw_addr))
}

