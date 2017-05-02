// Copyright 2017 The Gfx-rs Developers.
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

use core::{self as c, factory as f, handle, texture as t, format, shade, pso, buffer, mapping};
use core::memory::Bind;
use core::ShaderSet;
use std::sync::Arc;
use {Resources as R, Factory};

// TODO: dummy only
impl f::Factory<R> for Factory {
    fn get_capabilities(&self) -> &c::Capabilities { unimplemented!() }

    fn create_buffer_raw(&mut self, _: buffer::Info) -> Result<handle::RawBuffer<R>, buffer::CreationError> { unimplemented!() }
    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, _: buffer::Role, _: Bind)
                                   -> Result<handle::RawBuffer<R>, buffer::CreationError> { unimplemented!() }


    fn create_pipeline_state_raw(&mut self, _: &handle::Program<R>, _: &pso::Descriptor)
                                 -> Result<handle::RawPipelineState<R>, pso::CreationError> { unimplemented!() }
                                 
    fn create_program(&mut self, shader_set: &ShaderSet<R>)
                      -> Result<handle::Program<R>, shade::CreateProgramError> { unimplemented!() }
    
    fn create_shader(&mut self, stage: shade::Stage, code: &[u8]) ->
                     Result<handle::Shader<R>, shade::CreateShaderError> { unimplemented!() }

    fn create_sampler(&mut self, _: t::SamplerInfo) -> handle::Sampler<R> { unimplemented!() }

    fn read_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<R, T>)
                               -> Result<mapping::Reader<'b, R, T>,
                                         mapping::Error>
        where T: Copy { unimplemented!() }

    fn write_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<R, T>)
                                -> Result<mapping::Writer<'b, R, T>,
                                          mapping::Error>
        where T: Copy { unimplemented!() }

    fn create_texture_raw(&mut self, _: t::Info, _: Option<format::ChannelType>, _: Option<&[&[u8]]>)
                          -> Result<handle::RawTexture<R>, t::CreationError> { unimplemented!() }

    fn view_buffer_as_shader_resource_raw(&mut self, _: &handle::RawBuffer<R>)
        -> Result<handle::RawShaderResourceView<R>, f::ResourceViewError> { unimplemented!() }
    fn view_buffer_as_unordered_access_raw(&mut self, _: &handle::RawBuffer<R>)
        -> Result<handle::RawUnorderedAccessView<R>, f::ResourceViewError> { unimplemented!() }
    fn view_texture_as_shader_resource_raw(&mut self, _: &handle::RawTexture<R>, _: t::ResourceDesc)
        -> Result<handle::RawShaderResourceView<R>, f::ResourceViewError> { unimplemented!() }
    fn view_texture_as_unordered_access_raw(&mut self, _: &handle::RawTexture<R>)
        -> Result<handle::RawUnorderedAccessView<R>, f::ResourceViewError> { unimplemented!() }
    fn view_texture_as_render_target_raw(&mut self, _: &handle::RawTexture<R>, _: t::RenderDesc)
        -> Result<handle::RawRenderTargetView<R>, f::TargetViewError> { unimplemented!() }
    fn view_texture_as_depth_stencil_raw(&mut self, _: &handle::RawTexture<R>, _: t::DepthStencilDesc)
        -> Result<handle::RawDepthStencilView<R>, f::TargetViewError> { unimplemented!() }
}