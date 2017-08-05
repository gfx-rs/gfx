use core::{self as c, device as d, handle, texture as t, format, shade, pso, buffer, mapping};
use core::memory::Bind;
use core::ShaderSet;
use native;
use std::sync::Arc;
use {Backend as B, Device};

// TODO: dummy only
impl d::Device<B> for Device {
    fn get_capabilities(&self) -> &c::Capabilities { unimplemented!() }

    fn create_buffer_raw(&mut self, _: buffer::Info) -> Result<handle::RawBuffer<B>, buffer::CreationError> { unimplemented!() }
    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, _: buffer::Role, _: Bind)
                                   -> Result<handle::RawBuffer<B>, buffer::CreationError> { unimplemented!() }


    fn create_pipeline_state_raw(&mut self, _: &handle::Program<B>, _: &pso::Descriptor)
                                 -> Result<handle::RawPipelineState<B>, pso::CreationError> { unimplemented!() }

    fn create_program(&mut self, shader_set: &ShaderSet<B>)
                      -> Result<handle::Program<B>, shade::CreateProgramError> { unimplemented!() }

    fn create_shader(&mut self, stage: shade::Stage, code: &[u8]) ->
                     Result<handle::Shader<B>, shade::CreateShaderError> { unimplemented!() }

    fn create_sampler(&mut self, _: t::SamplerInfo) -> handle::Sampler<B> { unimplemented!() }

    fn create_semaphore(&mut self) -> handle::Semaphore<B> { unimplemented!() }

    fn create_fence(&mut self, _signalled: bool) -> handle::Fence<B> {
        unimplemented!()
    }

    fn reset_fences(&mut self, fences: &[&handle::Fence<B>]) {
        unimplemented!()
    }

    fn wait_for_fences(&mut self, _fences: &[&handle::Fence<B>], _wait: d::WaitFor, _timeout_ms: u32) -> bool {
        unimplemented!()
    }

    fn read_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<B, T>)
                               -> Result<mapping::Reader<'b, B, T>,
                                         mapping::Error>
        where T: Copy { unimplemented!() }

    fn write_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<B, T>)
                                -> Result<mapping::Writer<'b, B, T>,
                                          mapping::Error>
        where T: Copy { unimplemented!() }

    fn create_texture_raw(&mut self, _: t::Info, _: Option<format::ChannelType>, _: Option<&[&[u8]]>)
                          -> Result<handle::RawTexture<B>, t::CreationError> { unimplemented!() }

    fn view_buffer_as_shader_resource_raw(&mut self, _: &handle::RawBuffer<B>, _: format::Format)
        -> Result<handle::RawShaderResourceView<B>, d::ResourceViewError> { unimplemented!() }
    fn view_buffer_as_unordered_access_raw(&mut self, _: &handle::RawBuffer<B>)
        -> Result<handle::RawUnorderedAccessView<B>, d::ResourceViewError> { unimplemented!() }
    fn view_texture_as_shader_resource_raw(&mut self, _: &handle::RawTexture<B>, _: t::ResourceDesc)
        -> Result<handle::RawShaderResourceView<B>, d::ResourceViewError> { unimplemented!() }
    fn view_texture_as_unordered_access_raw(&mut self, _: &handle::RawTexture<B>)
        -> Result<handle::RawUnorderedAccessView<B>, d::ResourceViewError> { unimplemented!() }
    fn view_texture_as_render_target_raw(&mut self, _: &handle::RawTexture<B>, _: t::RenderDesc)
        -> Result<handle::RawRenderTargetView<B>, d::TargetViewError> { unimplemented!() }
    fn view_texture_as_depth_stencil_raw(&mut self, _: &handle::RawTexture<B>, _: t::DepthStencilDesc)
        -> Result<handle::RawDepthStencilView<B>, d::TargetViewError> { unimplemented!() }
}