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

use hal::{buffer, command, error, format, image, memory, query, pso, Features, Limits, QueueType};
use hal::{DrawCount, FrameImage, IndexCount, InstanceCount, VertexCount, VertexOffset, WorkGroupCount};
use hal::queue::{QueueFamilyId, Queues};
use hal::backend::RawQueueGroup;
use hal::range::RangeArg;

use winapi::shared::{dxgiformat, winerror};

use winapi::shared::dxgi::{IDXGIFactory, IDXGIAdapter, IDXGISwapChain};
use winapi::shared::minwindef::{FALSE, UINT};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{GetClientRect};
use winapi::um::{d3d11, d3dcommon};

use wio::com::ComPtr;

use std::ptr;
use std::mem;
use std::ops::Range;
use std::cell::RefCell;
use std::borrow::Borrow;

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

fn get_features(_device: ComPtr<d3d11::ID3D11Device>, _feature_level: d3dcommon::D3D_FEATURE_LEVEL) -> hal::Features {
    use hal::Features;

    let mut features =
        Features::ROBUST_BUFFER_ACCESS |
        Features::FULL_DRAW_INDEX_U32 |
        Features::FORMAT_BC;

    features
}

fn get_format_properties(device: ComPtr<d3d11::ID3D11Device>) -> [format::Properties; format::NUM_FORMATS] {
    let mut format_properties = [format::Properties::default(); format::NUM_FORMATS];
    for (i, props) in &mut format_properties.iter_mut().enumerate().skip(1) {
        let format: format::Format = unsafe { mem::transmute(i as u32) };

        let dxgi_format = match conv::map_format(format) {
            Some(format) => format,
            None => continue,
        };

        let mut support = d3d11::D3D11_FEATURE_DATA_FORMAT_SUPPORT {
            InFormat: dxgi_format,
            OutFormatSupport: 0,
        };
        let mut support_2 = d3d11::D3D11_FEATURE_DATA_FORMAT_SUPPORT2 {
            InFormat: dxgi_format,
            OutFormatSupport2: 0,
        };

        let hr = unsafe {
            device.CheckFeatureSupport(
                d3d11::D3D11_FEATURE_FORMAT_SUPPORT,
                &mut support as *mut _ as *mut _,
                mem::size_of::<d3d11::D3D11_FEATURE_DATA_FORMAT_SUPPORT>() as UINT
            )
        };

        if hr == winerror::S_OK {
            let can_buffer = 0 != support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_BUFFER;
            let can_image = 0 != support.OutFormatSupport & (
                d3d11::D3D11_FORMAT_SUPPORT_TEXTURE1D |
                d3d11::D3D11_FORMAT_SUPPORT_TEXTURE2D |
                d3d11::D3D11_FORMAT_SUPPORT_TEXTURE3D |
                d3d11::D3D11_FORMAT_SUPPORT_TEXTURECUBE
            );
            let can_linear = can_image && !format.surface_desc().is_compressed();
            if can_image {
                props.optimal_tiling |= format::ImageFeature::SAMPLED | format::ImageFeature::BLIT_SRC;
            }
            if can_linear {
                props.linear_tiling |= format::ImageFeature::SAMPLED | format::ImageFeature::BLIT_SRC;
            }
            if support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_IA_VERTEX_BUFFER != 0 {
                props.buffer_features |= format::BufferFeature::VERTEX;
            }
            if support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_SHADER_SAMPLE != 0 {
                props.optimal_tiling |= format::ImageFeature::SAMPLED_LINEAR;
            }
            if support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_RENDER_TARGET != 0 {
                props.optimal_tiling |= format::ImageFeature::COLOR_ATTACHMENT | format::ImageFeature::BLIT_DST;
                if can_linear {
                    props.linear_tiling |= format::ImageFeature::COLOR_ATTACHMENT | format::ImageFeature::BLIT_DST;
                }
            }
            if support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_BLENDABLE != 0 {
                props.optimal_tiling |= format::ImageFeature::COLOR_ATTACHMENT_BLEND;
            }
            if support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_DEPTH_STENCIL != 0 {
                props.optimal_tiling |= format::ImageFeature::DEPTH_STENCIL_ATTACHMENT;
            }
            if support.OutFormatSupport & d3d11::D3D11_FORMAT_SUPPORT_SHADER_LOAD != 0 {
                //TODO: check d3d12::D3D12_FORMAT_SUPPORT2_UAV_TYPED_LOAD ?
                if can_buffer {
                    props.buffer_features |= format::BufferFeature::UNIFORM_TEXEL;
                }
            }

            let hr = unsafe {
                device.CheckFeatureSupport(
                    d3d11::D3D11_FEATURE_FORMAT_SUPPORT2,
                    &mut support_2 as *mut _ as *mut _,
                    mem::size_of::<d3d11::D3D11_FEATURE_DATA_FORMAT_SUPPORT2>() as UINT
                )
            };
            if hr == winerror::S_OK {
                if support_2.OutFormatSupport2 & d3d11::D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_ADD != 0 {
                    //TODO: other atomic flags?
                    if can_buffer {
                        props.buffer_features |= format::BufferFeature::STORAGE_TEXEL_ATOMIC;
                    }
                    if can_image {
                        props.optimal_tiling |= format::ImageFeature::STORAGE_ATOMIC;
                    }
                }
                if support_2.OutFormatSupport2 & d3d11::D3D11_FORMAT_SUPPORT2_UAV_TYPED_STORE != 0 {
                    if can_buffer {
                        props.buffer_features |= format::BufferFeature::STORAGE_TEXEL;
                    }
                    if can_image {
                        props.optimal_tiling |= format::ImageFeature::STORAGE;
                    }
                }
            }
        }

        //TODO: blits, linear tiling
    }

    format_properties
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        let mut adapters = Vec::new();
        let mut idx = 0;

        while let Ok((adapter, info)) = dxgi::get_adapter(idx, self.factory.as_raw(), self.dxgi_version) {
            idx += 1;

            use hal::memory::Properties;

            // TODO: move into function?
            let (device, feature_level) = {
                let feature_level = get_feature_level(adapter.as_raw());

                let mut device = ptr::null_mut();
                let hr = unsafe {
                    d3d11::D3D11CreateDevice(
                        adapter.as_raw() as *mut _,
                        d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
                        ptr::null_mut(),
                        d3d11::D3D11_CREATE_DEVICE_DEBUG,
                        [feature_level].as_ptr(),
                        1,
                        d3d11::D3D11_SDK_VERSION,
                        &mut device as *mut *mut _ as *mut *mut _,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    )
                };

                if !winerror::SUCCEEDED(hr) {
                    continue;
                }

                (unsafe { ComPtr::<d3d11::ID3D11Device>::from_raw(device) }, feature_level)
            };

            let memory_properties = hal::MemoryProperties {
                memory_types: vec![
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL,
                        heap_index: 0,
                    },
                    hal::MemoryType {
                        properties: Properties::CPU_VISIBLE,
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
                max_vertex_input_attribute_offset: 255, // TODO
                max_vertex_input_attributes: d3d11::D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT as _,
                max_vertex_input_binding_stride: d3d11::D3D11_REQ_MULTI_ELEMENT_STRUCTURE_SIZE_IN_BYTES as _,
                max_vertex_input_bindings: d3d11::D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT as _, // TODO: verify same as attributes
                max_vertex_output_components: d3d11::D3D11_VS_OUTPUT_REGISTER_COUNT as _, // TODO
                min_buffer_copy_offset_alignment: 1,    // TODO
                min_buffer_copy_pitch_alignment: 1,     // TODO
                min_texel_buffer_offset_alignment: 1,   // TODO
                min_uniform_buffer_offset_alignment: 16, // TODO: verify
                min_storage_buffer_offset_alignment: 1, // TODO
                framebuffer_color_samples_count: 1,     // TODO
                framebuffer_depth_samples_count: 1,     // TODO
                framebuffer_stencil_samples_count: 1,   // TODO
                non_coherent_atom_size: 0,              // TODO
            };

            let features = get_features(device.clone(), feature_level);
            let format_properties = get_format_properties(device.clone());

            let physical_device = PhysicalDevice {
                adapter,
                features,
                limits,
                memory_properties,
                format_properties,
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
    #[derivative(Debug="ignore")]
    format_properties: [format::Properties; format::NUM_FORMATS],
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
        let idx = fmt.map(|fmt| fmt as usize).unwrap_or(0);
        self.format_properties[idx]
    }

    fn image_format_properties(&self, format: format::Format, dimensions: u8, tiling: image::Tiling, usage: image::Usage, storage_flags: image::StorageFlags) -> Option<image::FormatProperties> {
        conv::map_format(format)?; //filter out unknown formats

        let supported_usage = {
            use hal::image::Usage as U;
            let format_props = &self.format_properties[format as usize];
            let props = match tiling {
                image::Tiling::Optimal => format_props.optimal_tiling,
                image::Tiling::Linear => format_props.linear_tiling,
            };
            let mut flags = U::empty();
            // Note: these checks would have been nicer if we had explicit BLIT usage
            if props.contains(format::ImageFeature::BLIT_SRC) {
                flags |= U::TRANSFER_SRC;
            }
            if props.contains(format::ImageFeature::BLIT_DST) {
                flags |= U::TRANSFER_DST;
            }
            if props.contains(format::ImageFeature::SAMPLED) {
                flags |= U::SAMPLED;
            }
            if props.contains(format::ImageFeature::STORAGE) {
                flags |= U::STORAGE;
            }
            if props.contains(format::ImageFeature::COLOR_ATTACHMENT) {
                flags |= U::COLOR_ATTACHMENT;
            }
            if props.contains(format::ImageFeature::DEPTH_STENCIL_ATTACHMENT) {
                flags |= U::DEPTH_STENCIL_ATTACHMENT;
            }
            flags
        };
        if !supported_usage.contains(usage) {
            return None;
        }

        let max_resource_size = (d3d11::D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_A_TERM as usize) << 20;
        Some(match tiling {
            image::Tiling::Optimal => image::FormatProperties {
                max_extent: match dimensions {
                    1 => image::Extent {
                        width: d3d11::D3D11_REQ_TEXTURE1D_U_DIMENSION,
                        height: 1,
                        depth: 1,
                    },
                    2 => image::Extent {
                        width: d3d11::D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION,
                        height: d3d11::D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION,
                        depth: 1,
                    },
                    3 => image::Extent {
                        width: d3d11::D3D11_REQ_TEXTURE3D_U_V_OR_W_DIMENSION,
                        height: d3d11::D3D11_REQ_TEXTURE3D_U_V_OR_W_DIMENSION,
                        depth: d3d11::D3D11_REQ_TEXTURE3D_U_V_OR_W_DIMENSION,
                    },
                    _ => return None,
                },
                max_levels: d3d11::D3D11_REQ_MIP_LEVELS as _,
                max_layers: match dimensions {
                    1 => d3d11::D3D11_REQ_TEXTURE1D_ARRAY_AXIS_DIMENSION as _,
                    2 => d3d11::D3D11_REQ_TEXTURE2D_ARRAY_AXIS_DIMENSION as _,
                    _ => return None,
                },
                sample_count_mask: if dimensions == 2 && !storage_flags.contains(image::StorageFlags::CUBE_VIEW) &&
                    (usage.contains(image::Usage::COLOR_ATTACHMENT) | usage.contains(image::Usage::DEPTH_STENCIL_ATTACHMENT))
                {
                    0x3F //TODO: use D3D12_FEATURE_DATA_FORMAT_SUPPORT
                } else {
                    0x1
                },
                max_resource_size,
            },
            image::Tiling::Linear => image::FormatProperties {
                max_extent: match dimensions {
                    2 => image::Extent {
                        width: d3d11::D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION,
                        height: d3d11::D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION,
                        depth: 1,
                    },
                    _ => return None,
                },
                max_levels: 1,
                max_layers: 1,
                sample_count_mask: 0x1,
                max_resource_size,
            },
        })
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
    fn acquire_frame(&mut self, _sync: hal::FrameSync<Backend>) -> Result<hal::FrameImage, ()> {
        // TODO: non-`_DISCARD` swap effects have more than one buffer, `FLIP`
        //       effects are dxgi 1.3 (w10+?) in which case there is
        //       `GetCurrentBackBufferIndex()` on the swapchain
        Ok(0)
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

    fn present<IS, S, IW>(&mut self, swapchains: IS, _wait_semaphores: IW) -> Result<(), ()>
    where
        IS: IntoIterator<Item = (S, FrameImage)>,
        S: Borrow<Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<Semaphore>,
    {
        for (swapchain, _) in swapchains {
            unsafe { swapchain.borrow().dxgi_swapchain.Present(1, 0); }
        }

        Ok(())
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
                ibv.buffer.internal.raw,
                conv::map_index_type(ibv.index_type),
                ibv.offset as u32
            );
        }
    }

    fn bind_vertex_buffers(&mut self, first_binding: u32, vbs: pso::VertexBufferSet<Backend>) {
        let (buffers, offsets): (Vec<*mut d3d11::ID3D11Buffer>, Vec<u32>) = vbs.0.iter()
            .map(|(buf, offset)| (buf.internal.raw, *offset as u32))
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

    fn set_stencil_reference(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!()
    }

    fn set_stencil_read_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!();
    }

    fn set_stencil_write_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!();
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

    fn bind_graphics_descriptor_sets<'a, I, J>(&mut self, layout: &PipelineLayout, first_set: usize, sets: I, _offsets: J)
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<command::DescriptorSetOffset>,
    {
        for set in sets.into_iter() {
            let set = set.borrow();

            for (binding, cbv) in set.cbv_handles.borrow().iter() {
                unsafe { self.context.VSSetConstantBuffers(*binding, 1, cbv); }
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


    fn bind_compute_descriptor_sets<I, J>(&mut self, layout: &PipelineLayout, first_set: usize, sets: I, offsets: J)
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<command::DescriptorSetOffset>,
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
        assert_eq!(buffer.internal.srv.is_some(), true);

        for copy in regions.into_iter() {
            self.internal.copy_2d(
                self.context.clone(),
                buffer.internal.srv.unwrap(),
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

// Since we dont have any heaps to work with directly, everytime we bind a
// buffer/image to memory we allocate a dx11 resource and assign it a range.
//
// `HOST_VISIBLE` memory gets a staging buffer which covers the entire memory
// range. This forces us to only expose non-coherent memory, as this
// abstraction acts as a "cache" since the staging buffer is disjoint from all
// the dx11 resources we store in the struct.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Memory {
    properties: memory::Properties,
    size: u64,

    // stores flushed ranges inbetween mappings
    flushes: RefCell<Vec<Range<u64>>>,

    // list of all buffers bound to this memory
    #[derivative(Debug="ignore")]
    local_buffers: RefCell<Vec<(Range<u64>, InternalBuffer)>>,

    // staging buffer covering the whole memory region, if it's HOST_VISIBLE
    #[derivative(Debug="ignore")]
    host_buffer: Option<ComPtr<d3d11::ID3D11Buffer>>,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

impl Memory {
    pub fn flush(&self, range: Range<u64>) {
        self.flushes.borrow_mut().push(range);
    }

    pub fn bind_buffer(&self, range: Range<u64>, buffer: InternalBuffer) {
        self.local_buffers.borrow_mut().push((range, buffer));
    }
}

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
pub struct InternalBuffer {
    raw: *mut d3d11::ID3D11Buffer,
    srv: Option<*mut d3d11::ID3D11ShaderResourceView>
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer {
    #[derivative(Debug="ignore")]
    internal: InternalBuffer,
    size: u64,
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
    cbv_handles: RefCell<Vec<(u32, *mut d3d11::ID3D11Buffer)>>,
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
