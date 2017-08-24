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

//! Dummy backend implementation to test the code for compile errors
//! outside of the graphics development environment.

use {Adapter, AdapterInfo, Backend, Capabilities, Resources, IndexType, VertexCount, QueueType,
     Gpu, Device, CommandQueue, QueueFamily, ShaderSet, Surface, Swapchain,
     Frame, FrameSync, SwapchainConfig, Backbuffer, WindowExt, RawSubmission};
use {buffer, format, state, target, handle, mapping, pool, pso, shade, texture};
use command::{self, AccessInfo};
use device::{ResourceViewError, TargetViewError, WaitFor};
use memory::Bind;

/// Dummy backend.
pub enum DummyBackend { }
impl Backend for DummyBackend {
    type Adapter = DummyAdapter;
    type CommandQueue = DummyQueue;
    type Device = DummyDevice;
    type QueueFamily = DummyFamily;
    type Resources = DummyResources;
    type SubmitInfo = DummySubmitInfo;

    type RawCommandBuffer = DummyCommandBuffer;
    type SubpassCommandBuffer = DummySubpassCommandBuffer;

    type RawCommandPool = DummyRawCommandPool;
    type SubpassCommandPool = DummySubpassCommandPool;
}

/// Dummy adapter.
pub struct DummyAdapter;
impl Adapter<DummyBackend> for DummyAdapter {
    fn open(&self, _: &[(&DummyFamily, QueueType, u32)]) -> Gpu<DummyBackend> {
        unimplemented!()
    }

    fn get_info(&self) -> &AdapterInfo {
        unimplemented!()
    }

    fn get_queue_families(&self) -> &[(DummyFamily, QueueType)] {
        unimplemented!()
    }
}

/// Dummy command queue doing nothing.
pub struct DummyQueue;
impl CommandQueue<DummyBackend> for DummyQueue {
    unsafe fn submit_raw<'a, I>(
        &mut self,
        _: I,
        _: Option<&handle::Fence<DummyResources>>,
        _: &AccessInfo<DummyResources>,
    ) where I: Iterator<Item=RawSubmission<'a, DummyBackend>> {
        unimplemented!()
    }

    fn pin_submitted_resources(&mut self, _: &handle::Manager<DummyResources>) {
        unimplemented!()
    }

    fn cleanup(&mut self) {
        unimplemented!()
    }
}

/// Dummy device doing nothing.
pub struct DummyDevice;
impl Device<DummyResources> for DummyDevice {
    fn get_capabilities(&self) -> &Capabilities {
        unimplemented!()
    }
    fn create_buffer_raw(
        &mut self,
        _: buffer::Info,
    ) -> Result<handle::RawBuffer<DummyResources>, buffer::CreationError> {
        unimplemented!()
    }
    fn create_buffer_immutable_raw(
        &mut self,
        _: &[u8],
        _: usize,
        _: buffer::Role,
        _: Bind,
    ) -> Result<handle::RawBuffer<DummyResources>, buffer::CreationError> {
        unimplemented!()
    }
    fn create_pipeline_state_raw(
        &mut self,
        _: &handle::Program<DummyResources>,
        _: &pso::Descriptor,
    ) -> Result<handle::RawPipelineState<DummyResources>, pso::CreationError> {
        unimplemented!()
    }

    fn create_program(
        &mut self,
        _: &ShaderSet<DummyResources>,
    ) -> Result<handle::Program<DummyResources>, shade::CreateProgramError> {
        unimplemented!()
    }

    fn create_shader(
        &mut self,
        _: shade::Stage,
        _: &[u8],
    ) -> Result<handle::Shader<DummyResources>, shade::CreateShaderError> {
        unimplemented!()
    }

    fn create_sampler(&mut self, _: texture::SamplerInfo) -> handle::Sampler<DummyResources> {
        unimplemented!()
    }
    fn create_semaphore(&mut self) -> handle::Semaphore<DummyResources> {
        unimplemented!()
    }
    fn create_fence(&mut self, _: bool) -> handle::Fence<DummyResources> {
        unimplemented!()
    }
    fn reset_fences(&mut self, _: &[&handle::Fence<DummyResources>]) {
        unimplemented!()
    }
    fn wait_for_fences(
        &mut self,
        _: &[&handle::Fence<DummyResources>],
        _: WaitFor,
        _: u32,
    ) -> bool {
        unimplemented!()
    }

    fn read_mapping<'a, 'b, T>(
        &'a mut self,
        _: &'b handle::Buffer<DummyResources, T>,
    ) -> Result<mapping::Reader<'b, DummyResources, T>, mapping::Error>
    where
        T: Copy,
    {
        unimplemented!()
    }

    fn write_mapping<'a, 'b, T>(
        &'a mut self,
        _: &'b handle::Buffer<DummyResources, T>,
    ) -> Result<mapping::Writer<'b, DummyResources, T>, mapping::Error>
    where
        T: Copy,
    {
        unimplemented!()
    }

    fn create_texture_raw(
        &mut self,
        _: texture::Info,
        _: Option<format::ChannelType>,
        _: Option<&[&[u8]]>,
    ) -> Result<handle::RawTexture<DummyResources>, texture::CreationError> {
        unimplemented!()
    }

    fn view_buffer_as_shader_resource_raw(
        &mut self,
        _: &handle::RawBuffer<DummyResources>,
        _: format::Format,
    ) -> Result<handle::RawShaderResourceView<DummyResources>, ResourceViewError> {
        unimplemented!()
    }
    fn view_buffer_as_unordered_access_raw(
        &mut self,
        _: &handle::RawBuffer<DummyResources>,
    ) -> Result<handle::RawUnorderedAccessView<DummyResources>, ResourceViewError> {
        unimplemented!()
    }
    fn view_texture_as_shader_resource_raw(
        &mut self,
        _: &handle::RawTexture<DummyResources>,
        _: texture::ResourceDesc,
    ) -> Result<handle::RawShaderResourceView<DummyResources>, ResourceViewError> {
        unimplemented!()
    }
    fn view_texture_as_unordered_access_raw(
        &mut self,
        _: &handle::RawTexture<DummyResources>,
    ) -> Result<handle::RawUnorderedAccessView<DummyResources>, ResourceViewError> {
        unimplemented!()
    }
    fn view_texture_as_render_target_raw(
        &mut self,
        _: &handle::RawTexture<DummyResources>,
        _: texture::RenderDesc,
    ) -> Result<handle::RawRenderTargetView<DummyResources>, TargetViewError> {
        unimplemented!()
    }
    fn view_texture_as_depth_stencil_raw(
        &mut self,
        _: &handle::RawTexture<DummyResources>,
        _: texture::DepthStencilDesc,
    ) -> Result<handle::RawDepthStencilView<DummyResources>, TargetViewError> {
        unimplemented!()
    }
}

/// Dummy queue family;
pub struct DummyFamily;
impl QueueFamily for DummyFamily {
    fn num_queues(&self) -> u32 {
        unimplemented!()
    }
}

/// Dummy submit info containing nothing.
#[derive(Clone)]
pub struct DummySubmitInfo;

/// Dummy subpass command buffer.
pub struct DummySubpassCommandBuffer;
impl command::CommandBuffer<DummyBackend> for DummySubpassCommandBuffer {
    unsafe fn end(&mut self) -> DummySubmitInfo {
        unimplemented!()
    }
}

/// Dummy raw command pool.
pub struct DummyRawCommandPool;
impl pool::RawCommandPool<DummyBackend> for DummyRawCommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, _: usize) {
        unimplemented!()
    }

    unsafe fn from_queue<Q>(_: Q, _: usize) -> Self
    where
        Q: AsRef<DummyQueue>,
    {
        unimplemented!()
    }

    unsafe fn acquire_command_buffer(&mut self) -> &mut DummyCommandBuffer {
        unimplemented!()
    }
}

/// Dummy subpass command pool.
pub struct DummySubpassCommandPool;
impl pool::SubpassCommandPool<DummyBackend> for DummySubpassCommandPool {}

/// Dummy resources phantom type
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DummyResources {}

impl Resources for DummyResources {
    type Buffer               = ();
    type Shader               = ();
    type Program              = ();
    type PipelineStateObject  = ();
    type Texture              = ();
    type ShaderResourceView   = ();
    type UnorderedAccessView  = ();
    type RenderTargetView     = ();
    type DepthStencilView     = ();
    type Sampler              = ();
    type Semaphore            = ();
    type Fence                = DummyFence;
    type Mapping              = DummyMapping;
}

/// Dummy fence that does nothing.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DummyFence;

/// Dummy mapping which will crash on use.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DummyMapping;

impl mapping::Gate<DummyResources> for DummyMapping {
    unsafe fn set<T>(&self, _index: usize, _val: T) {
        unimplemented!()
    }
    unsafe fn slice<'a, 'b, T>(&'a self, _len: usize) -> &'b [T] {
        unimplemented!()
    }
    unsafe fn mut_slice<'a, 'b, T>(&'a self, _len: usize) -> &'b mut [T] {
        unimplemented!()
    }
}

/// Dummy command buffer, which ignores all the calls.
pub struct DummyCommandBuffer;
impl command::CommandBuffer<DummyBackend> for DummyCommandBuffer {
    unsafe fn end(&mut self) -> DummySubmitInfo {
        unimplemented!()
    }
}
impl command::Buffer<DummyResources> for DummyCommandBuffer {
    fn reset(&mut self) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<DummyResources>) {}
    fn bind_constant_buffers(&mut self, _: &[pso::ConstantBufferParam<DummyResources>]) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: &[pso::ResourceViewParam<DummyResources>]) {}
    fn bind_unordered_views(&mut self, _: &[pso::UnorderedViewParam<DummyResources>]) {}
    fn bind_samplers(&mut self, _: &[pso::SamplerParam<DummyResources>]) {}
    fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<DummyResources>) {}
    fn bind_index(&mut self, _: (), _: IndexType) {}
    fn set_scissor(&mut self, _: target::Rect) {}
    fn set_ref_values(&mut self, _: state::RefValues) {}
    fn copy_buffer(&mut self, _: (), _: (), _: usize, _: usize, _: usize) {}
    fn copy_buffer_to_texture(
        &mut self,
        _: (),
        _: usize,
        _: (),
        _: texture::Kind,
        _: Option<texture::CubeFace>,
        _: texture::RawImageInfo,
    ) {
    }
    fn copy_texture_to_buffer(
        &mut self,
        _: (),
        _: texture::Kind,
        _: Option<texture::CubeFace>,
        _: texture::RawImageInfo,
        _: (),
        _: usize,
    ) {
    }
    fn update_buffer(&mut self, _: (), _: &[u8], _: usize) {}
    fn update_texture(
        &mut self,
        _: (),
        _: texture::Kind,
        _: Option<texture::CubeFace>,
        _: &[u8],
        _: texture::RawImageInfo,
    ) {
    }
    fn generate_mipmap(&mut self, _: ()) {}
    fn clear_color(&mut self, _: (), _: command::ClearColor) {}
    fn clear_depth_stencil(&mut self, _: (), _: Option<target::Depth>, _: Option<target::Stencil>) {
    }
    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: Option<command::InstanceParams>) {}
    fn call_draw_indexed(
        &mut self,
        _: VertexCount,
        _: VertexCount,
        _: VertexCount,
        _: Option<command::InstanceParams>,
    ) {
    }
}

/// Dummy surface.
pub struct DummySurface;
impl Surface<DummyBackend> for DummySurface {
    type Swapchain = DummySwapchain;

    fn supports_queue(&self, _: &DummyFamily) -> bool {
        unimplemented!()
    }

    fn build_swapchain<Q>(&mut self, _: SwapchainConfig, _: &Q) -> Self::Swapchain
    where
        Q: AsRef<DummyQueue>,
    {
        unimplemented!()
    }
}

/// Dummy swapchain.
pub struct DummySwapchain;
impl Swapchain<DummyBackend> for DummySwapchain {
    fn get_backbuffers(&mut self) -> &[Backbuffer<DummyBackend>] {
        unimplemented!()
    }

    fn acquire_frame(&mut self, sync: FrameSync<DummyResources>) -> Frame {
        unimplemented!()
    }

    fn present<Q: AsMut<DummyQueue>>(
        &mut self,
        _: &mut Q,
        _: &[&handle::Semaphore<DummyResources>],
    ) {
        unimplemented!()
    }
}

/// Dummy window.
pub struct DummyWindow;
impl WindowExt<DummyBackend> for DummyWindow {
    type Surface = DummySurface;
    type Adapter = DummyAdapter;

    fn get_surface_and_adapters(&mut self) -> (DummySurface, Vec<DummyAdapter>) {
        unimplemented!()
    }
}
