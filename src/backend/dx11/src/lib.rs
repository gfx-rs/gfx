//#[deny(missing_docs)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate derivative;
extern crate gfx_hal as hal;
#[macro_use]
extern crate log;
extern crate smallvec;
extern crate spirv_cross;
extern crate winapi;
#[cfg(feature = "winit")]
extern crate winit;
extern crate wio;

use hal::{buffer, command, device, error, format, image, memory, mapping, queue, query, pool, pso, pass, Features, Limits, QueueType};
use hal::{IndexCount, IndexType, InstanceCount, VertexCount, VertexOffset, WorkGroupCount};
use hal::queue::{QueueFamily as HalQueueFamily, QueueFamilyId, Queues};
use hal::format::Aspects;
use hal::range::RangeArg;

use winapi::shared::{dxgi, dxgi1_2, dxgi1_3, dxgi1_4, winerror};
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winnt::{LARGE_INTEGER};
use winapi::um::winuser::{GetClientRect};
use winapi::um::{d3d11, d3dcommon};

use wio::com::ComPtr;

use std::ptr;
use std::mem;
use std::ops::Range;
use std::borrow::{BorrowMut, Borrow};

use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;
use std::os::raw::c_void;

pub struct Instance {
    pub(crate) factory: ComPtr<dxgi::IDXGIFactory1>
}

unsafe impl Send for Instance { }
unsafe impl Sync for Instance { }

impl Instance {
    pub fn create(_: &str, _: u32) -> Self {
        // TODO: get the latest factory we can find

        let mut factory: *mut dxgi::IDXGIFactory1 = ptr::null_mut();
        let hr = unsafe {
            dxgi::CreateDXGIFactory1(
                &dxgi::IID_IDXGIFactory1,
                &mut factory as *mut *mut _ as *mut *mut _
            )
        };

        if !winerror::SUCCEEDED(hr) {
            error!("Failed on dxgi factory creation: {:?}", hr);
        }

        Instance {
            factory: unsafe { ComPtr::from_raw(factory) }
        }
    }

    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        let (width, height) = unsafe {
            let mut rect: RECT = mem::zeroed();
            if GetClientRect(hwnd as *mut _, &mut rect as *mut RECT) == 0 {
                panic!("GetClientRect failed");
            }
            ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32)
        };

        Surface {
            factory: self.factory.clone(),
            wnd_handle: hwnd as *mut _,
            width: width,
            height: height,
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::windows::WindowExt;
        self.create_surface_from_hwnd(window.get_hwnd() as *mut _)
}
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        let mut adapters = Vec::new();
        let mut idx = 0;

        loop {
            let adapter = {
                let mut adapter: *mut dxgi::IDXGIAdapter1 = ptr::null_mut();
                let hr = unsafe {
                    self.factory.EnumAdapters1(idx, &mut adapter as *mut *mut _)
                };

                if hr == winerror::DXGI_ERROR_NOT_FOUND {
                    break;
                }

                unsafe { ComPtr::from_raw(adapter) }
            };

            idx += 1;
            let mut umd: LARGE_INTEGER = unsafe { mem::zeroed() };

            // check if adapter supports creating d3d11 device
            let mut desc: dxgi::DXGI_ADAPTER_DESC1 = unsafe { mem::zeroed() };
            unsafe { adapter.GetDesc1(&mut desc); }

            let device_name = {
                let len = desc.Description.iter().take_while(|&&c| c != 0).count();
                let name = <OsString as OsStringExt>::from_wide(&desc.Description[..len]);
                name.to_string_lossy().into_owned()
            };

            let info = hal::AdapterInfo {
                name: device_name,
                vendor: desc.VendorId as usize,
                device: desc.DeviceId as usize,
                software_rendering: (desc.Flags & dxgi::DXGI_ADAPTER_FLAG_SOFTWARE) != 0,
            };

            let physical_device = PhysicalDevice {
                adapter,
                // TODO: check for features
                features: hal::Features::empty(),
                limits: hal::Limits {
                    max_texture_size: d3d11::D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION as _,
                    max_patch_size: 0, // TODO
                    max_viewports: d3d11::D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as _,
                    max_compute_group_count: [
                        d3d11::D3D11_CS_THREAD_GROUP_MAX_X,
                        d3d11::D3D11_CS_THREAD_GROUP_MAX_Y,
                        d3d11::D3D11_CS_THREAD_GROUP_MAX_Z
                    ],
                    max_compute_group_size: [
                        d3d11::D3D11_CS_THREAD_GROUP_MAX_THREADS_PER_GROUP,
                        1,
                        1
                    ], // TODO
                    min_buffer_copy_offset_alignment: 0,    // TODO
                    min_buffer_copy_pitch_alignment: 0,     // TODO
                    min_texel_buffer_offset_alignment: 0,   // TODO
                    min_uniform_buffer_offset_alignment: 0, // TODO
                    min_storage_buffer_offset_alignment: 0, // TODO
                    framebuffer_color_samples_count: 0,     // TODO
                    framebuffer_depth_samples_count: 0,     // TODO
                    framebuffer_stencil_samples_count: 0,   // TODO
                    non_coherent_atom_size: 0,              // TODO
                },
                memory_properties: hal::MemoryProperties {
                    memory_types: vec![], // TODO
                    memory_heaps: vec![]  // TODO
                }
            };

            adapters.push(hal::Adapter {
                info,
                physical_device,
                queue_families: vec![QueueFamily]
            });
        }

        adapters
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PhysicalDevice {
    #[derivative(Debug="ignore")]
    adapter: ComPtr<dxgi::IDXGIAdapter1>,
    features: hal::Features,
    limits: hal::Limits,
    memory_properties: hal::MemoryProperties,
}

unsafe impl Send for PhysicalDevice { }
unsafe impl Sync for PhysicalDevice { }

// TODO: does the adapter we get earlier matter for feature level?
fn get_feature_level(adapter: *mut dxgi::IDXGIAdapter) -> d3dcommon::D3D_FEATURE_LEVEL {
    let requested_feature_levels = [
        d3dcommon::D3D_FEATURE_LEVEL_11_1,
        d3dcommon::D3D_FEATURE_LEVEL_11_0,
        d3dcommon::D3D_FEATURE_LEVEL_10_1,
        d3dcommon::D3D_FEATURE_LEVEL_10_0,
        d3dcommon::D3D_FEATURE_LEVEL_9_3,
        d3dcommon::D3D_FEATURE_LEVEL_9_2,
        d3dcommon::D3D_FEATURE_LEVEL_9_1,
    ];

    let mut feature_level = d3dcommon::D3D_FEATURE_LEVEL_9_1;
    let hr = unsafe {
        d3d11::D3D11CreateDevice(
            adapter,
            d3dcommon::D3D_DRIVER_TYPE_NULL,
            ptr::null_mut(),
            0,
            requested_feature_levels[..].as_ptr(),
            requested_feature_levels.len() as _,
            d3d11::D3D11_SDK_VERSION,
            ptr::null_mut(),
            &mut feature_level as *mut _,
            ptr::null_mut()
        )
    };

    if !winerror::SUCCEEDED(hr) {
        // if there is no 11.1 runtime installed, requesting
        // `D3D_FEATURE_LEVEL_11_1` will return E_INVALIDARG so we just retry
        // without that
        if hr == winerror::E_INVALIDARG {
            let hr = unsafe {
                d3d11::D3D11CreateDevice(
                    ptr::null_mut(),
                    d3dcommon::D3D_DRIVER_TYPE_NULL,
                    ptr::null_mut(),
                    0,
                    requested_feature_levels[1..].as_ptr(),
                    (requested_feature_levels.len() - 1) as _,
                    d3d11::D3D11_SDK_VERSION,
                    ptr::null_mut(),
                    &mut feature_level as *mut _,
                    ptr::null_mut()
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: device might not support any feature levels?
                unimplemented!();
            }
        }
    }

    feature_level
}

// TODO: PhysicalDevice
impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(&self, families: &[(&QueueFamily, &[hal::QueuePriority])])
        -> Result<hal::Gpu<Backend>, error::DeviceCreationError>
    {
        let (device, cxt) = {
            let feature_level = get_feature_level(self.adapter.as_raw() as *mut _);
            let mut returned_level = d3dcommon::D3D_FEATURE_LEVEL_9_1;

            // TODO: request debug device only on debug config?
            let mut device = ptr::null_mut();
            let mut cxt = ptr::null_mut();
            let hr = unsafe {
                d3d11::D3D11CreateDevice(
                    self.adapter.as_raw() as *mut _,
                    d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
                    ptr::null_mut(),
                    d3d11::D3D11_CREATE_DEVICE_DEBUG,
                    [feature_level].as_ptr(),
                    1,
                    d3d11::D3D11_SDK_VERSION,
                    &mut device as *mut *mut _ as *mut *mut _,
                    &mut returned_level as *mut _,
                    &mut cxt as *mut *mut _ as *mut *mut _,
                )
            };

            // NOTE: returns error if adapter argument is non-null and driver
            // type is not unknown; or if debug device is requested but not
            // present
            if !winerror::SUCCEEDED(hr) {
                return Err(error::DeviceCreationError::InitializationFailed);
            }

            println!("feature level={:x}", feature_level);

            unsafe { (ComPtr::from_raw(device), ComPtr::from_raw(cxt)) }
        };

        let device = Device::new(device, cxt);

        Ok(hal::Gpu {
            device,
            queues
        })
    }

    fn format_properties(&self, fmt: Option<format::Format>) -> format::Properties {
        unimplemented!()
    }


    fn image_format_properties(&self, _format: format::Format, dimensions: u8, tiling: image::Tiling, usage: image::Usage, storage_flags: image::StorageFlags) -> Option<image::FormatProperties> {
        unimplemented!()
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        use hal::memory::Properties;

        hal::MemoryProperties {
            memory_types: vec![
                hal::MemoryType {
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 0
                }
            ],
            memory_heaps: vec![!0]
        }
    }

    fn features(&self) -> Features {
        self.features
    }

    fn limits(&self) -> Limits {
        self.limits
    }

}



pub struct Device {
    device: ComPtr<d3d11::ID3D11Device>,
    context: ComPtr<d3d11::ID3D11DeviceContext>
}

unsafe impl Send for Device { }
unsafe impl Sync for Device { }

impl Device {
    fn new(device: ComPtr<d3d11::ID3D11Device>, context: ComPtr<d3d11::ID3D11DeviceContext>) -> Self {
        Device {
            device,
            context
        }
    }
}

impl hal::Device<Backend> for Device {
    fn allocate_memory(
        &self,
        mem_type: hal::MemoryTypeId,
        size: u64,
    ) -> Result<Memory, device::OutOfMemory> {
        unimplemented!()
    }

    fn create_command_pool(
        &self, family: QueueFamilyId, _create_flags: pool::CommandPoolCreateFlags
    ) -> CommandPool {
        unimplemented!()
    }

    fn destroy_command_pool(&self, _pool: CommandPool) {
        unimplemented!()
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        unimplemented!()
    }

    fn create_pipeline_layout<IS, IR>(
        &self,
        sets: IS,
        push_constant_ranges: IR,
    ) -> PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        unimplemented!()
    }

    fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, Backend>,
    ) -> Result<GraphicsPipeline, pso::CreationError> {
        unimplemented!()
    }

    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, Backend>,
    ) -> Result<ComputePipeline, pso::CreationError> {
        unimplemented!()
    }

    fn create_framebuffer<I>(
        &self,
        _renderpass: &RenderPass,
        attachments: I,
        extent: image::Extent,
    ) -> Result<Framebuffer, device::FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<ImageView>
    {
        unimplemented!()
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<ShaderModule, device::ShaderError> {
        unimplemented!()
    }

    fn create_buffer(
        &self,
        mut size: u64,
        usage: buffer::Usage,
    ) -> Result<UnboundBuffer, buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&self, buffer: &UnboundBuffer) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_buffer_memory(
        &self,
        memory: &Memory,
        offset: u64,
        buffer: UnboundBuffer,
    ) -> Result<Buffer, device::BindError> {
        unimplemented!()
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        buffer: &Buffer,
        format: Option<format::Format>,
        range: R,
    ) -> Result<BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        flags: image::StorageFlags,
    ) -> Result<UnboundImage, image::CreationError> {
        unimplemented!()
    }

    fn get_image_requirements(&self, image: &UnboundImage) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_image_memory(
        &self,
        memory: &Memory,
        offset: u64,
        image: UnboundImage,
    ) -> Result<Image, device::BindError> {
        unimplemented!()
    }

    fn create_image_view(
        &self,
        image: &Image,
        view_kind: image::ViewKind,
        format: format::Format,
        _swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<ImageView, image::ViewError> {
        unimplemented!()
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> Sampler {
        unimplemented!()
    }

    fn create_descriptor_pool<I>(
        &self,
        max_sets: usize,
        descriptor_pools: I,
    ) -> DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>
    {
        unimplemented!()
    }

    fn create_descriptor_set_layout<I>(
        &self,
        bindings: I,
    )-> DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>
    {
        unimplemented!()
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        unimplemented!()
    }

    fn copy_descriptor_sets<'a, I>(&self, copy_iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, Backend>>,
    {
        unimplemented!()
    }

    fn map_memory<R>(&self, memory: &Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    fn unmap_memory(&self, memory: &Memory) {
        unimplemented!()
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a Memory, R)>,
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a Memory, R)>,
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    fn create_semaphore(&self) -> Semaphore {
        unimplemented!()
    }

    fn create_fence(&self, signalled: bool) -> Fence {
        unimplemented!()
    }

    fn reset_fence(&self, fence: &Fence) {
        unimplemented!()
    }

    fn wait_for_fences<I>(&self, fences: I, wait: device::WaitFor, timeout_ms: u32) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<Fence>,
    {
        unimplemented!()
    }

    fn get_fence_status(&self, _fence: &Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, memory: Memory) {
        unimplemented!()
    }

    fn create_query_pool(&self, query_ty: query::QueryType, count: u32) -> QueryPool {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _pool: QueryPool) {
        unimplemented!()
    }

    fn destroy_shader_module(&self, shader_lib: ShaderModule) {
        unimplemented!()
    }

    fn destroy_render_pass(&self, _rp: RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&self, layout: PipelineLayout) {
        unimplemented!()
    }

    fn destroy_graphics_pipeline(&self, pipeline: GraphicsPipeline) {
        unimplemented!()
    }

    fn destroy_compute_pipeline(&self, pipeline: ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&self, _fb: Framebuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&self, buffer: Buffer) {
        unimplemented!()
    }

    fn destroy_buffer_view(&self, _view: BufferView) {
        unimplemented!()
    }

    fn destroy_image(&self, image: Image) {
        unimplemented!()
    }

    fn destroy_image_view(&self, _view: ImageView) {
        unimplemented!()
    }

    fn destroy_sampler(&self, _sampler: Sampler) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&self, pool: DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&self, _layout: DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&self, _fence: Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&self, _semaphore: Semaphore) {
        unimplemented!()
    }

    fn create_swapchain(
        &self,
        surface: &mut Surface,
        config: hal::SwapchainConfig,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        unimplemented!()
    }

    fn destroy_swapchain(&self, _swapchain: Swapchain) {
        unimplemented!()
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }

}

pub struct Surface {
    pub(crate) factory: ComPtr<dxgi::IDXGIFactory1>,
    wnd_handle: HWND,
    width: u32,
    height: u32
}

unsafe impl Send for Surface { }
unsafe impl Sync for Surface { }

impl hal::Surface<Backend> for Surface {
    fn supports_queue_family(&self, queue_family: &QueueFamily) -> bool {
        true
        /*match queue_family {
            &QueueFamily::Present => true,
            _ => false
        }*/
    }

    // TODO: stereo swapchain?
    fn kind(&self) -> image::Kind {
        image::Kind::D2(self.width, self.height, 1, 1)
    }

    fn capabilities_and_formats(&self, _: &PhysicalDevice) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>) {
        let extent = hal::window::Extent2D {
            width: self.width,
            height: self.height,
        };

        // TODO: flip swap effects require dx11.1/windows8
        // NOTE: some swap effects affect msaa capabilities..
        let capabilities = hal::SurfaceCapabilities {
            image_count: 2..16, // TODO:
            current_extent: Some(extent),
            extents: extent..extent,
            max_image_layers: 1,
        };

        let formats = vec![
            format::Format::Bgra8Srgb,
            format::Format::Bgra8Unorm,
            format::Format::Rgba8Srgb,
            format::Format::Rgba8Unorm,
            format::Format::A2b10g10r10Unorm,
            format::Format::Rgba16Float,
        ];

        (capabilities, Some(formats))
    }

}

pub struct Swapchain(());
unsafe impl Send for Swapchain { }
unsafe impl Sync for Swapchain { }

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _sync: hal::FrameSync<Backend>) -> hal::Frame {
        unimplemented!()
    }
}


#[derive(Debug, Clone, Copy)]
pub struct QueueFamily;

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType { QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}

#[derive(Debug)]
pub struct CommandQueue(());
impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(&mut self, submission: hal::queue::RawSubmission<Backend, IC>, fence: Option<&Fence>)
    where
        IC: IntoIterator,
        IC::Item: Borrow<CommandBuffer>,
    {
        unimplemented!()
    }

    fn present<IS, IW>(&mut self, swapchains: IS, _wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<Semaphore>,
    {
        unimplemented!()
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }

}

#[derive(Debug, Clone)]
pub struct CommandBuffer(());
impl hal::command::RawCommandBuffer<Backend> for CommandBuffer {

    fn begin(&mut self, _flags: command::CommandBufferFlags, _info: command::CommandBufferInheritanceInfo<Backend>) {

        unimplemented!()
    }

    fn finish(&mut self) {
        unimplemented!()
    }

    fn reset(&mut self, _release_resources: bool) {
        unimplemented!()
    }

    fn begin_render_pass<T>(&mut self, render_pass: &RenderPass, framebuffer: &Framebuffer, target_rect: pso::Rect, clear_values: T, _first_subpass: command::SubpassContents)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ClearValueRaw>,
    {
        unimplemented!()
    }

    fn next_subpass(&mut self, _contents: command::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        unimplemented!()
    }

    fn pipeline_barrier<'a, T>(&mut self, _stages: Range<pso::PipelineStage>, _dependencies: memory::Dependencies, barriers: T)
    where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        unimplemented!()
    }

    fn clear_image<T>(&mut self, image: &Image, _: image::Layout, color: command::ClearColorRaw, depth_stencil: command::ClearDepthStencilRaw, subresource_ranges: T)
    where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        unimplemented!()
    }

    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<command::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::Rect>,
    {
        unimplemented!()
    }

    fn resolve_image<T>(&mut self, src: &Image, _src_layout: image::Layout, dst: &Image, _dst_layout: image::Layout, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageResolve>,
    {
        unimplemented!()
    }

    fn blit_image<T>(&mut self, _src: &Image, _src_layout: image::Layout, _dst: &Image, _dst_layout: image::Layout, _filter: image::Filter, _regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageBlit>
    {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<Backend>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, first_binding: u32, vbs: pso::VertexBufferSet<Backend>) {
        unimplemented!()
    }

    fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        unimplemented!()
    }

    fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        unimplemented!()
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        unimplemented!()
    }

    fn set_stencil_reference(&mut self, front: pso::StencilValue, back: pso::StencilValue) {
        unimplemented!()
    }

    fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &GraphicsPipeline) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets<'a, T>(&mut self, layout: &PipelineLayout, first_set: usize, sets: T)
    where
        T: IntoIterator,
        T::Item: Borrow<DescriptorSet>,
    {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pipeline: &ComputePipeline) {
        unimplemented!()
    }


    fn bind_compute_descriptor_sets<T>(&mut self, layout: &PipelineLayout, first_set: usize, sets: T)
    where
        T: IntoIterator,
        T::Item: Borrow<DescriptorSet>,
    {
        unimplemented!()
    }

    fn dispatch(&mut self, count: WorkGroupCount) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset) {
        unimplemented!()
    }

    fn fill_buffer<R>(&mut self, buffer: &Buffer, range: R, data: u32)
    where
        R: RangeArg<buffer::Offset>,
    {
        unimplemented!()
    }

    fn update_buffer(&mut self, _buffer: &Buffer, _offset: buffer::Offset, _data: &[u8]) {
        unimplemented!()
    }

    fn copy_buffer<T>(&mut self, src: &Buffer, dst: &Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferCopy>,
    {
        unimplemented!()
    }

    fn copy_image<T>(&mut self, src: &Image, _: image::Layout, dst: &Image, _: image::Layout, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageCopy>,
    {
        unimplemented!()
    }

    fn copy_buffer_to_image<T>(&mut self, buffer: &Buffer, image: &Image, _: image::Layout, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn copy_image_to_buffer<T>(&mut self, image: &Image, _: image::Layout, buffer: &Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        unimplemented!()
    }

    fn draw_indexed(&mut self, indices: Range<IndexCount>, base_vertex: VertexOffset, instances: Range<InstanceCount>) {
        unimplemented!()
    }

    fn draw_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset, draw_count: u32, stride: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset, draw_count: u32, stride: u32) {
        unimplemented!()
    }

    fn begin_query(&mut self, query: query::Query<Backend>, flags: query::QueryControl) {
        unimplemented!()
    }

    fn end_query(&mut self, query: query::Query<Backend>) {
        unimplemented!()
    }

    fn reset_query_pool(&mut self, _pool: &QueryPool, _queries: Range<query::QueryId>) {
        unimplemented!()
    }

    fn write_timestamp(&mut self, _: pso::PipelineStage, query: query::Query<Backend>) {
        unimplemented!()
    }

    fn push_graphics_constants(&mut self, layout: &PipelineLayout, _stages: pso::ShaderStageFlags, offset: u32, constants: &[u32]) {
        unimplemented!()
    }

    fn push_compute_constants(&mut self, layout: &PipelineLayout, offset: u32, constants: &[u32]) {
        unimplemented!()
    }

    fn execute_commands<I>(&mut self, buffers: I)
    where
        I: IntoIterator,
        I::Item: Borrow<CommandBuffer>,
    {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Memory(());

pub struct CommandPool(());
impl hal::pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    fn allocate(&mut self, num: usize, level: command::RawLevel) -> Vec<CommandBuffer> {
        unimplemented!()
    }

    unsafe fn free(&mut self, _cbufs: Vec<CommandBuffer>) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct ShaderModule(());
#[derive(Debug)]
pub struct RenderPass(());
#[derive(Debug)]
pub struct Framebuffer(());

#[derive(Debug)]
pub struct UnboundBuffer(());
#[derive(Debug)]
pub struct Buffer(());
#[derive(Debug)]
pub struct BufferView(());
#[derive(Debug)]
pub struct UnboundImage(());
#[derive(Debug)]
pub struct Image(());
#[derive(Debug)]
pub struct ImageView(());
#[derive(Debug)]
pub struct Sampler(());

#[derive(Debug)]
pub struct ComputePipeline(());
#[derive(Debug)]
pub struct GraphicsPipeline(());
#[derive(Debug)]
pub struct PipelineLayout(());
#[derive(Debug)]
pub struct DescriptorSetLayout(());

// TODO: descriptor pool
#[derive(Debug)]
pub struct DescriptorPool(());
impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(&mut self, layout: &DescriptorSetLayout) -> Result<DescriptorSet, pso::AllocationError> {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct DescriptorSet(());

#[derive(Debug)]
pub struct Fence(());
#[derive(Debug)]
pub struct Semaphore(());
#[derive(Debug)]
pub struct QueryPool(());

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = CommandQueue;
    type CommandBuffer = CommandBuffer;

    type Memory = Memory;
    type CommandPool = CommandPool;

    type ShaderModule = ShaderModule;
    type RenderPass = RenderPass;
    type Framebuffer = Framebuffer;

    type UnboundBuffer = UnboundBuffer;
    type Buffer = Buffer;
    type BufferView = BufferView;
    type UnboundImage = UnboundImage;
    type Image = Image;
    type ImageView = ImageView;
    type Sampler = Sampler;

    type ComputePipeline = ComputePipeline;
    type GraphicsPipeline = GraphicsPipeline;
    type PipelineLayout = PipelineLayout;
    type DescriptorSetLayout = DescriptorSetLayout;
    type DescriptorPool = DescriptorPool;
    type DescriptorSet = DescriptorSet;

    type Fence = Fence;
    type Semaphore = Semaphore;
    type QueryPool = QueryPool;
}
