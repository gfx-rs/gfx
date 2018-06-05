//#[deny(missing_docs)]

//#[macro_use]
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

use hal::{buffer, command, error, format, image, memory, mapping, query, pool, pso, pass, Features, Limits, QueueType};
use hal::{DrawCount, IndexCount, InstanceCount, VertexCount, VertexOffset, WorkGroupCount};
use hal::queue::{QueueFamilyId, Queues};
use hal::backend::RawQueueGroup;
use hal::range::RangeArg;

use winapi::shared::{dxgiformat, dxgitype, winerror};

use winapi::shared::dxgi::{DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD, IDXGIFactory, IDXGIAdapter, IDXGISwapChain};
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{GetClientRect};
use winapi::um::{d3d11, d3d11sdklayers, d3dcommon};

use wio::com::ComPtr;

use std::ptr;
use std::mem;
use std::ops::Range;
use std::sync::Arc;
use std::cell::RefCell;
use std::borrow::{BorrowMut, Borrow};
use std::collections::BTreeMap;

use std::os::raw::c_void;

mod conv;
mod dxgi;
mod shader;
mod internal;
mod device;

#[derive(Clone, Debug)]
pub(crate) struct ViewInfo {
    resource: *mut d3d11::ID3D11Resource,
    kind: image::Kind,
    flags: image::StorageFlags,
    view_kind: image::ViewKind,
    format: dxgiformat::DXGI_FORMAT,
    range: image::SubresourceRange,
}

pub struct Instance {
    pub(crate) factory: ComPtr<IDXGIFactory>,
    pub(crate) dxgi_version: dxgi::DxgiVersion
}

unsafe impl Send for Instance { }
unsafe impl Sync for Instance { }

impl Instance {
    pub fn create(_: &str, _: u32) -> Self {
        // TODO: get the latest factory we can find

        let (factory, dxgi_version) = dxgi::get_dxgi_factory().unwrap();

        info!("DXGI version: {:?}", dxgi_version);

        Instance {
            factory,
            dxgi_version
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

        while let Ok((adapter, info)) = dxgi::get_adapter(idx, self.factory.as_raw(), self.dxgi_version) {
            idx += 1;

            use hal::memory::Properties;

            // TODO: we should improve the way memory is managed. we should
            //       give access to DEFAULT, DYNAMIC and STAGING;
            //
            //       roughly this should translate to:
            //
            //       DEFAULT => DEVICE_LOCAL
            //
            //       NOTE: DYNAMIC only offers cpu write, potentially add
            //             a HOST_WRITE_ONLY flag..
            //       DYNAMIC => DEVICE_LOCAL | CPU_VISIBLE
            //
            //       STAGING => CPU_VISIBLE | CPU_CACHED
            let memory_properties = hal::MemoryProperties {
                memory_types: vec![
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL,
                        heap_index: 0,
                    },
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL, //| Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                        heap_index: 0,
                    },
                    hal::MemoryType {
                        properties: Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                        heap_index: 1,
                    },
                ],
                // TODO: would using *VideoMemory and *SystemMemory from
                //       DXGI_ADAPTER_DESC be too optimistic? :)
                memory_heaps: vec![!0, !0]
            };

            let limits = hal::Limits {
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
                max_vertex_input_attribute_offset: 0, // TODO
                max_vertex_input_attributes: 0, // TODO
                max_vertex_input_binding_stride: 0, // TODO
                max_vertex_input_bindings: 0, // TODO
                max_vertex_output_components: 0, // TODO
                min_buffer_copy_offset_alignment: 1,    // TODO
                min_buffer_copy_pitch_alignment: 1,     // TODO
                min_texel_buffer_offset_alignment: 1,   // TODO
                min_uniform_buffer_offset_alignment: 1, // TODO
                min_storage_buffer_offset_alignment: 1, // TODO
                framebuffer_color_samples_count: 0,     // TODO
                framebuffer_depth_samples_count: 0,     // TODO
                framebuffer_stencil_samples_count: 0,   // TODO
                non_coherent_atom_size: 0,              // TODO
            };

            let physical_device = PhysicalDevice {
                adapter,
                // TODO: check for features
                features: hal::Features::empty(),
                limits,
                memory_properties
            };

            info!("{:#?}", info);

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
    adapter: ComPtr<IDXGIAdapter>,
    features: hal::Features,
    limits: hal::Limits,
    memory_properties: hal::MemoryProperties,
}

unsafe impl Send for PhysicalDevice { }
unsafe impl Sync for PhysicalDevice { }

// TODO: does the adapter we get earlier matter for feature level?
fn get_feature_level(adapter: *mut IDXGIAdapter) -> d3dcommon::D3D_FEATURE_LEVEL {
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
            d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
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
                    adapter,
                    d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
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
            let feature_level = get_feature_level(self.adapter.as_raw());
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

            info!("feature level={:x}", feature_level);

            unsafe { (ComPtr::from_raw(device), ComPtr::from_raw(cxt)) }
        };

        let device = device::Device::new(device, cxt, self.memory_properties.clone());
        
        // TODO: deferred context => 1 cxt/queue?
        let queues = Queues::new(
            families
                .into_iter()
                .map(|&(family, prio)| {
                    assert_eq!(prio.len(), 1);
                    let mut group = RawQueueGroup::new(family.clone());

                    // TODO: multiple queues?
                    let queue = CommandQueue {
                        context: device.context.clone(),
                    };
                    group.add_queue(queue);
                    (QueueFamilyId(0), group)
                })
                .collect()
        );

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
        self.memory_properties.clone()
    }

    fn features(&self) -> Features {
        self.features
    }

    fn limits(&self) -> Limits {
        self.limits
    }

}

pub struct Surface {
    pub(crate) factory: ComPtr<IDXGIFactory>,
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
        // TODO: _DISCARD swap effects can only have one image?
        let capabilities = hal::SurfaceCapabilities {
            image_count: 1..16, // TODO:
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

pub struct Swapchain {
    dxgi_swapchain: ComPtr<IDXGISwapChain>,
}

unsafe impl Send for Swapchain { }
unsafe impl Sync for Swapchain { }

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _sync: hal::FrameSync<Backend>) -> hal::Frame {
        // TODO: non-`_DISCARD` swap effects have more than one buffer, `FLIP`
        //       effects are dxgi 1.3 (w10+?) in which case there is
        //       `GetCurrentBackBufferIndex()` on the swapchain

        hal::Frame::new(0)
    }
}


#[derive(Debug, Clone, Copy)]
pub struct QueueFamily;

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType { QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct CommandQueue {
    #[derivative(Debug="ignore")]
    context: ComPtr<d3d11::ID3D11DeviceContext>
}

unsafe impl Send for CommandQueue { }
unsafe impl Sync for CommandQueue { }

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(&mut self, submission: hal::queue::RawSubmission<Backend, IC>, fence: Option<&Fence>)
    where
        IC: IntoIterator,
        IC::Item: Borrow<CommandBuffer>,
    {
        for cmd_buf in submission.cmd_buffers.into_iter() {
            let cmd_buf = cmd_buf.borrow();
            self.context.ExecuteCommandList(cmd_buf.as_raw_list().as_raw(), FALSE);
        }
    }

    fn present<IS, IW>(&mut self, swapchains: IS, _wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<Semaphore>,
    {
        for swapchain in swapchains {
            unsafe { swapchain.borrow().dxgi_swapchain.Present(1, 0); }
        }
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }

}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct CommandBuffer {
    // TODO: better way of sharing
    #[derivative(Debug="ignore")]
    internal: internal::BufferImageCopy,
    #[derivative(Debug="ignore")]
    context: ComPtr<d3d11::ID3D11DeviceContext>,
    #[derivative(Debug="ignore")]
    list: Option<ComPtr<d3d11::ID3D11CommandList>>
}

unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {} 

impl CommandBuffer {
    fn create_deferred(device: ComPtr<d3d11::ID3D11Device>, internal: internal::BufferImageCopy) -> Self {
        let mut context: *mut d3d11::ID3D11DeviceContext = ptr::null_mut();
        let hr = unsafe {
            device.CreateDeferredContext(0, &mut context as *mut *mut _ as *mut *mut _)
        };

        CommandBuffer {
            internal,
            context: unsafe { ComPtr::from_raw(context) },
            list: None
        }
    }

    fn as_raw_list(&self) -> ComPtr<d3d11::ID3D11CommandList> {
        self.list.clone().unwrap().clone()
    }
}

impl hal::command::RawCommandBuffer<Backend> for CommandBuffer {

    fn begin(&mut self, _flags: command::CommandBufferFlags, _info: command::CommandBufferInheritanceInfo<Backend>) {

        // TODO:
    }

    fn finish(&mut self) {
        // TODO:

        let mut list = ptr::null_mut();
        let hr = unsafe { self.context.FinishCommandList(FALSE, &mut list as *mut *mut _ as *mut *mut _) };
        self.list = Some(unsafe { ComPtr::from_raw(list) });
    }

    fn reset(&mut self, _release_resources: bool) {
        unimplemented!()
    }

    fn begin_render_pass<T>(&mut self, render_pass: &RenderPass, framebuffer: &Framebuffer, target_rect: pso::Rect, clear_values: T, _first_subpass: command::SubpassContents)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ClearValueRaw>,
    {
        // TODO: very temp

        let color_views = framebuffer.attachments.iter()
            .filter(|a| a.rtv_handle.is_some())
            .map(|a| a.rtv_handle.clone().unwrap().as_raw())
            .collect::<Vec<_>>();

        let depth_view = framebuffer.attachments.iter().find(|a| a.dsv_handle.is_some());

        
        unsafe {
            for (clear, view) in clear_values.into_iter().zip(framebuffer.attachments.iter()) {
                let clear = clear.borrow();

                if let Some(ref handle) = view.rtv_handle {
                    self.context.ClearRenderTargetView(handle.clone().as_raw(), &clear.color.float32);
                }

                if let Some(ref handle) = view.dsv_handle {
                    self.context.ClearDepthStencilView(handle.clone().as_raw(), d3d11::D3D11_CLEAR_DEPTH, clear.depth_stencil.depth, 0);
                }
            }

            self.context.OMSetRenderTargets(
                color_views.len() as _,
                color_views.as_ptr(),
                if let Some(depth_attachment) = depth_view {
                    depth_attachment.dsv_handle.clone().unwrap().as_raw()
                } else {
                    ptr::null_mut()
                },
            );
        }
        // TODO: begin render pass
        //unimplemented!()
    }

    fn next_subpass(&mut self, _contents: command::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        // TODO: end render pass
        //unimplemented!()
    }

    fn pipeline_barrier<'a, T>(&mut self, _stages: Range<pso::PipelineStage>, _dependencies: memory::Dependencies, barriers: T)
    where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        // TODO: should we track and assert on resource states?
        // unimplemented!()
    }

    fn clear_image<T>(&mut self, image: &Image, _: image::Layout, color: command::ClearColorRaw, depth_stencil: command::ClearDepthStencilRaw, subresource_ranges: T)
    where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        // TODO: use a internal program to clear for subregions in the image
        for subresource_range in subresource_ranges {
            let _sub = subresource_range.borrow();
            unsafe {
                self.context.ClearRenderTargetView(
                    image.rtv.clone().unwrap().as_raw(),
                    &color.float32
                );
            }
        }
    }

    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<command::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
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
        unsafe {
            self.context.IASetIndexBuffer(
                ibv.buffer.device_local_buffer().as_raw(),
                conv::map_index_type(ibv.index_type),
                ibv.offset as u32
            );
        }
    }

    fn bind_vertex_buffers(&mut self, first_binding: u32, vbs: pso::VertexBufferSet<Backend>) {
        let (buffers, offsets): (Vec<*mut d3d11::ID3D11Buffer>, Vec<u32>) = vbs.0.iter()
            .map(|(buf, offset)| (buf.device_local_buffer().as_raw(), *offset as u32))
            .unzip();

        // TODO: strides
        let strides = [32u32; 16];

        unsafe {
            self.context.IASetVertexBuffers(
                first_binding,
                buffers.len() as _,
                buffers.as_ptr(),
                strides.as_ptr(),
                offsets.as_ptr(),
            );
        }
    }

    fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        let viewports = viewports.into_iter().map(|v| {
            let v = v.borrow();
            conv::map_viewport(v)
        }).collect::<Vec<_>>();

        unsafe { self.context.RSSetViewports(viewports.len() as _, viewports.as_ptr()); }
    }

    fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        let scissors = scissors.into_iter().map(|s| {
            let s = s.borrow();
            conv::map_rect(s)
        }).collect::<Vec<_>>();

        unsafe { self.context.RSSetScissorRects(scissors.len() as _, scissors.as_ptr()); }
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

    fn set_line_width(&mut self, width: f32) {
        validate_line_width(width);
    }

    fn set_depth_bias(&mut self, _depth_bias: pso::DepthBias) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &GraphicsPipeline) {
        unsafe {
            self.context.IASetPrimitiveTopology(pipeline.topology);
            self.context.IASetInputLayout(pipeline.input_layout.as_raw());

            self.context.VSSetShader(pipeline.vs.as_raw(), ptr::null_mut(), 0);
            if let Some(ref ps) = pipeline.ps {
                self.context.PSSetShader(ps.as_raw(), ptr::null_mut(), 0);
            }

            self.context.RSSetState(pipeline.rasterizer_state.as_raw());

            // TODO: blend constants
            self.context.OMSetBlendState(pipeline.blend_state.as_raw(), &[1f32; 4], !0);
            if let Some(ref state) = pipeline.depth_stencil_state {
                // TODO stencil
                self.context.OMSetDepthStencilState(state.as_raw(), 0);
            }
        }
    }

    fn bind_graphics_descriptor_sets<'a, T>(&mut self, layout: &PipelineLayout, first_set: usize, sets: T)
    where
        T: IntoIterator,
        T::Item: Borrow<DescriptorSet>,
    {
        for set in sets.into_iter() {
            let set = set.borrow();

            for (binding, cbv) in set.cbv_handles.borrow().iter() {
                let cbv = cbv.as_raw();
                unsafe { self.context.VSSetConstantBuffers(*binding, 1, &cbv); }
            }

            for (binding, srv) in set.srv_handles.borrow().iter() {
                let srv = srv.as_raw();
                unsafe { self.context.PSSetShaderResources(*binding, 1, &srv); }
            }

            for (binding, sampler) in set.sampler_handles.borrow().iter() {
                let sampler = sampler.as_raw();
                unsafe { self.context.PSSetSamplers(*binding, 1, &sampler); }
            }
        }
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
        for copy in regions.into_iter() {
            self.internal.copy_2d(
                self.context.clone(),
                buffer.srv.clone().unwrap(),
                image.uav.clone().unwrap(),
                copy.borrow().clone()
            );
        }
    }

    fn copy_image_to_buffer<T>(&mut self, image: &Image, _: image::Layout, buffer: &Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        unsafe {
            self.context.DrawInstanced(
                vertices.end - vertices.start,
                instances.end - instances.start,
                vertices.start,
                instances.start,
            );
        }
    }

    fn draw_indexed(&mut self, indices: Range<IndexCount>, base_vertex: VertexOffset, instances: Range<InstanceCount>) {
        unsafe {
            self.context.DrawIndexedInstanced(
                indices.end - indices.start,
                instances.end - instances.start,
                indices.start,
                base_vertex,
                instances.start,
            );
        }
    }

    fn draw_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset, draw_count: DrawCount, stride: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset, draw_count: DrawCount, stride: u32) {
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

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Memory {
    properties: memory::Properties,
    #[derivative(Debug="ignore")]
    // TODO: :-(
    buffer: RefCell<Option<InternalBuffer>>,
    size: u64,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {} 

pub struct CommandPool {
    device: ComPtr<d3d11::ID3D11Device>,
    internal: internal::BufferImageCopy,
}

unsafe impl Send for CommandPool {}
unsafe impl Sync for CommandPool {} 

impl hal::pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        //unimplemented!()
    }

    fn allocate(&mut self, num: usize, level: command::RawLevel) -> Vec<CommandBuffer> {
        (0..num)
            .map(|_| CommandBuffer::create_deferred(self.device.clone(), self.internal.clone()))
            .collect()
    }

    unsafe fn free(&mut self, _cbufs: Vec<CommandBuffer>) {
        unimplemented!()
    }
}

/// Similarily to dx12 backend, we can handle either precompiled dxbc or spirv
// TODO: derivative doesn't work on enum variants?
//#[derive(Derivative)]
//#[derivative(Debug)]
pub enum ShaderModule {
    Dxbc(Vec<u8>),
    Spirv(Vec<u8>)
}

// TODO: temporary
impl ::std::fmt::Debug for ShaderModule {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", "ShaderModule { ... }")
    }
}

unsafe impl Send for ShaderModule { }
unsafe impl Sync for ShaderModule { }

#[derive(Debug)]
pub struct RenderPass;
#[derive(Debug)]
pub struct Framebuffer {
    attachments: Vec<ImageView>,
    layers: image::Layer,
}

#[derive(Debug)]
pub struct UnboundBuffer {
    usage: buffer::Usage,
    bind: d3d11::D3D11_BIND_FLAG,
    size: u64,
    requirements: memory::Requirements,
}

#[derive(Clone)]
pub enum InternalBuffer {
    Coherent(ComPtr<d3d11::ID3D11Buffer>),
    NonCoherent {
        device: ComPtr<d3d11::ID3D11Buffer>,
        staging: ComPtr<d3d11::ID3D11Buffer>
    }
}

impl InternalBuffer {
    pub fn device_local_buffer(&self) -> ComPtr<d3d11::ID3D11Buffer> {
        match self {
            InternalBuffer::Coherent(ref buf) => buf.clone(),
            InternalBuffer::NonCoherent { ref device, ref staging } => device.clone()
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer {
    #[derivative(Debug="ignore")]
    buffer: InternalBuffer,
    #[derivative(Debug="ignore")]
    srv: Option<ComPtr<d3d11::ID3D11ShaderResourceView>>,
    size: u64,
}

impl Buffer {
    pub fn device_local_buffer(&self) -> ComPtr<d3d11::ID3D11Buffer> {
        self.buffer.device_local_buffer()
    }
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {} 

#[derive(Debug)]
pub struct BufferView;
#[derive(Debug)]
pub struct UnboundImage {
    kind: image::Kind,
    mip_levels: image::Level,
    format: format::Format,
    tiling: image::Tiling,
    usage: image::Usage,
    flags: image::StorageFlags,
    bind: d3d11::D3D11_BIND_FLAG,
    requirements: memory::Requirements
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Image {
    #[derivative(Debug="ignore")]
    resource: *mut d3d11::ID3D11Resource,
    kind: image::Kind,
    usage: image::Usage,
    storage_flags: image::StorageFlags,
    dxgi_format: dxgiformat::DXGI_FORMAT,
    bytes_per_block: u8,
    block_dim: (u8, u8),
    num_levels: image::Level,
    #[derivative(Debug="ignore")]
    uav: Option<ComPtr<d3d11::ID3D11UnorderedAccessView>>,
    #[derivative(Debug="ignore")]
    rtv: Option<ComPtr<d3d11::ID3D11RenderTargetView>>
}

unsafe impl Send for Image { }
unsafe impl Sync for Image { }

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct ImageView {
    #[derivative(Debug="ignore")]
    rtv_handle: Option<ComPtr<d3d11::ID3D11RenderTargetView>>,
    #[derivative(Debug="ignore")]
    srv_handle: Option<ComPtr<d3d11::ID3D11ShaderResourceView>>,
    #[derivative(Debug="ignore")]
    dsv_handle: Option<ComPtr<d3d11::ID3D11DepthStencilView>>,
    #[derivative(Debug="ignore")]
    uav_handle: Option<ComPtr<d3d11::ID3D11UnorderedAccessView>>,
}

unsafe impl Send for ImageView { }
unsafe impl Sync for ImageView { }

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Sampler {
    #[derivative(Debug="ignore")]
    sampler_handle: ComPtr<d3d11::ID3D11SamplerState>,
}

unsafe impl Send for Sampler { }
unsafe impl Sync for Sampler { }

#[derive(Debug)]
pub struct ComputePipeline;

/// NOTE: some objects are hashed internally and reused when created with the
///       same params[0], need to investigate which interfaces this applies
///       to.
///
/// [0]: https://msdn.microsoft.com/en-us/library/windows/desktop/ff476500(v=vs.85).aspx
#[derive(Derivative)]
#[derivative(Debug)]
pub struct GraphicsPipeline {
    // TODO: gs, hs, ds
    #[derivative(Debug="ignore")]
    vs: ComPtr<d3d11::ID3D11VertexShader>,
    #[derivative(Debug="ignore")]
    ps: Option<ComPtr<d3d11::ID3D11PixelShader>>,
    #[derivative(Debug="ignore")]
    topology: d3d11::D3D11_PRIMITIVE_TOPOLOGY,
    #[derivative(Debug="ignore")]
    input_layout: ComPtr<d3d11::ID3D11InputLayout>,
    #[derivative(Debug="ignore")]
    rasterizer_state: ComPtr<d3d11::ID3D11RasterizerState>,
    #[derivative(Debug="ignore")]
    blend_state: ComPtr<d3d11::ID3D11BlendState>,
    #[derivative(Debug="ignore")]
    depth_stencil_state: Option<ComPtr<d3d11::ID3D11DepthStencilState>>,
}

unsafe impl Send for GraphicsPipeline { }
unsafe impl Sync for GraphicsPipeline { }

#[derive(Debug)]
pub struct PipelineLayout;

#[derive(Debug)]
pub struct DescriptorSetLayout {
    bindings: Vec<pso::DescriptorSetLayoutBinding>,
}

// TODO: descriptor pool
#[derive(Debug)]
pub struct DescriptorPool;
impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(&mut self, layout: &DescriptorSetLayout) -> Result<DescriptorSet, pso::AllocationError> {
        // TODO: actually look at the layout maybe..
        Ok(DescriptorSet::new())
    }

    fn free_sets(&mut self, descriptor_sets: &[DescriptorSet]) {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct DescriptorSet {
    // TODO: need to handle arrays and stage flags
    #[derivative(Debug="ignore")]
    srv_handles: RefCell<Vec<(u32, ComPtr<d3d11::ID3D11ShaderResourceView>)>>,
    #[derivative(Debug="ignore")]
    cbv_handles: RefCell<Vec<(u32, ComPtr<d3d11::ID3D11Buffer>)>>,
    #[derivative(Debug="ignore")]
    sampler_handles: RefCell<Vec<(u32, ComPtr<d3d11::ID3D11SamplerState>)>>,
}

unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {} 

impl DescriptorSet {
    pub fn new() -> Self {
        DescriptorSet {
            srv_handles: RefCell::new(Vec::new()),
            cbv_handles: RefCell::new(Vec::new()),
            sampler_handles: RefCell::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
pub struct Fence;
#[derive(Debug)]
pub struct Semaphore;
#[derive(Debug)]
pub struct QueryPool;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = device::Device;

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

fn validate_line_width(width: f32) {
    // Note from the Vulkan spec:
    // > If the wide lines feature is not enabled, lineWidth must be 1.0
    // Simply assert and no-op because DX11 never exposes `Features::LINE_WIDTH`
    assert_eq!(width, 1.0);
}
