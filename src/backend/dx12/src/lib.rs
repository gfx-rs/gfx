#[macro_use]
extern crate bitflags;
extern crate d3d12;
extern crate d3dcompiler;
extern crate dxguid;
extern crate dxgi;
extern crate gfx_hal as core;
extern crate kernel32;
#[macro_use]
extern crate log;
extern crate smallvec;
extern crate spirv_cross;
extern crate user32;
extern crate winapi;
#[cfg(feature = "winit")]
extern crate winit;
extern crate wio;

mod command;
mod conv;
mod device;
mod free_list;
mod native;
mod pool;
mod shade;
mod window;

use core::{memory, Features, Limits, QueueType};
use wio::com::ComPtr;

use std::{mem, ptr};
use std::os::raw::c_void;
use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct QueueFamily;

impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 { 1 } // TODO: infinite software queues actually
}

/// Create associated command queues for a specific queue type
fn collect_queues<C>(
     queue_descs: &[(&QueueFamily, QueueType, u32)],
     device: &Device,
     collect_type: QueueType,
) -> Vec<core::CommandQueue<Backend, C>> {
    queue_descs.iter()
        .filter(|&&(_, qtype, _)| qtype == collect_type)
        .flat_map(|&(_, _, qcount)| {
            (0..qcount).map(|_| {
                let mut device_raw = device.raw.clone();
                let mut queue = ptr::null_mut();
                let qtype = match collect_type {
                    QueueType::General | QueueType::Graphics => winapi::D3D12_COMMAND_LIST_TYPE_DIRECT,
                    QueueType::Compute => winapi::D3D12_COMMAND_LIST_TYPE_COMPUTE,
                    QueueType::Transfer => winapi::D3D12_COMMAND_LIST_TYPE_COPY,
                };

                let queue_desc = winapi::D3D12_COMMAND_QUEUE_DESC {
                    Type: qtype,
                    Priority: 0,
                    Flags: winapi::D3D12_COMMAND_QUEUE_FLAG_NONE,
                    NodeMask: 0,
                };

                let hr = unsafe {
                    device_raw.CreateCommandQueue(
                        &queue_desc,
                        &dxguid::IID_ID3D12CommandQueue,
                        &mut queue as *mut *mut _ as *mut *mut c_void,
                    )
                };

                if !winapi::SUCCEEDED(hr) {
                    error!("error on queue creation: {:x}", hr);
                }

                unsafe {
                    core::CommandQueue::new(
                        CommandQueue {
                            raw: ComPtr::new(queue),
                            device: device_raw,
                            list_type: qtype,
                        }
                    )
                }
            })
        }).collect()
}

#[derive(Clone)]
pub struct Adapter {
    adapter: ComPtr<winapi::IDXGIAdapter2>,
    info: core::AdapterInfo,
    queue_families: Vec<(QueueFamily, QueueType)>,
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, QueueType, u32)]) -> core::Gpu<Backend> {
        // Create D3D12 device
        let mut device_raw = ptr::null_mut();
        let hr = unsafe {
            d3d12::D3D12CreateDevice(
                self.adapter.as_mut() as *mut _ as *mut winapi::IUnknown,
                winapi::D3D_FEATURE_LEVEL_12_0, // TODO: correct feature level?
                &dxguid::IID_ID3D12Device,
                &mut device_raw as *mut *mut _ as *mut *mut c_void,
            )
        };
        if !winapi::SUCCEEDED(hr) {
            error!("error on device creation: {:x}", hr);
        }
        let device = Device::new(unsafe { ComPtr::new(device_raw) });

        // https://msdn.microsoft.com/en-us/library/windows/desktop/dn788678(v=vs.85).aspx
        let base_memory_types = [
            core::MemoryType {
                id: 0,
                properties: memory::DEVICE_LOCAL,
                heap_index: 1,
            },
            core::MemoryType {
                id: 1,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::MemoryType {
                id: 2,
                properties: memory::CPU_VISIBLE | memory::WRITE_COMBINED,
                heap_index: 0,
            },
        ];
        let memory_types = if device.private_caps.heterogeneous_resource_heaps {
            base_memory_types.to_vec()
        } else {
            // the bit pattern of ID becomes 0bTTII, where
            // TT=1 for buffers, TT=2 for images, and TT=3 for targets
            // TT=0 is reserved for future use, helps to avoid ambiguity
            // and II is the same `id` as the `base_memory_types` have
            let mut types = Vec::new();
            for &tt in &[1, 2, 3] {
                types.extend(base_memory_types
                    .iter()
                    .map(|&mt| core::MemoryType {
                        id: mt.id + (tt << 2),
                        .. mt
                    })
                );
            }
            types
        };

        core::Gpu {
            general_queues: collect_queues(queue_descs, &device, QueueType::General),
            graphics_queues: collect_queues(queue_descs, &device, QueueType::Graphics),
            compute_queues: collect_queues(queue_descs, &device, QueueType::Compute),
            transfer_queues: collect_queues(queue_descs, &device, QueueType::Transfer),
            memory_types,
            memory_heaps: vec![!0, !0], // TODO: VRAM sizes
            device,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.info
    }

    fn get_queue_families(&self) -> &[(QueueFamily, QueueType)] {
        &self.queue_families
    }
}

pub struct CommandQueue {
    pub(crate) raw: ComPtr<winapi::ID3D12CommandQueue>,
    device: ComPtr<winapi::ID3D12Device>,
    list_type: winapi::D3D12_COMMAND_LIST_TYPE,
}
unsafe impl Send for CommandQueue {} //blocked by ComPtr

impl core::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(
        &mut self,
        submission: core::RawSubmission<Backend>,
        fence: Option<&native::Fence>,
    ) {
        // TODO: semaphores
        let mut lists = submission
            .cmd_buffers
            .iter()
            .map(|buf| buf.as_raw_list())
            .collect::<Vec<_>>();
        self.raw.ExecuteCommandLists(lists.len() as _, lists.as_mut_ptr());

        if let Some(fence) = fence {
            assert_eq!(winapi::S_OK,
                self.raw.Signal(fence.raw.as_mut(), 1)
            );
        }
    }
}

#[derive(Clone)]
pub struct Capabilities {
    heterogeneous_resource_heaps: bool,
}

#[derive(Clone)]
pub struct Device {
    raw: ComPtr<winapi::ID3D12Device>,
    features: core::Features,
    limits: core::Limits,
    private_caps: Capabilities,
    // CPU only pools
    rtv_pool: Arc<Mutex<native::DescriptorCpuPool>>,
    dsv_pool: Arc<Mutex<native::DescriptorCpuPool>>,
    srv_pool: Arc<Mutex<native::DescriptorCpuPool>>,
    uav_pool: Arc<Mutex<native::DescriptorCpuPool>>,
    sampler_pool: Arc<Mutex<native::DescriptorCpuPool>>,
    // CPU/GPU descriptor heaps
    heap_srv_cbv_uav: Arc<Mutex<native::DescriptorHeap>>,
    heap_sampler: Arc<Mutex<native::DescriptorHeap>>,
    events: Vec<winapi::HANDLE>,
}
unsafe impl Send for Device {} //blocked by ComPtr

impl Device {
    fn new(mut device: ComPtr<winapi::ID3D12Device>) -> Device {
        let mut features: winapi::D3D12_FEATURE_DATA_D3D12_OPTIONS = unsafe { mem::zeroed() };
        assert_eq!(winapi::S_OK, unsafe {
            device.CheckFeatureSupport(winapi::D3D12_FEATURE_D3D12_OPTIONS,
                &mut features as *mut _ as *mut _,
                mem::size_of::<winapi::D3D12_FEATURE_DATA_D3D12_OPTIONS>() as _)
        });

        // Allocate descriptor heaps
        let max_rtvs = 256; // TODO
        let rtv_pool = native::DescriptorCpuPool {
            heap: Self::create_descriptor_heap_impl(
                &mut device,
                winapi::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                false,
                max_rtvs,
            ),
            offset: 0,
            size: 0,
            max_size: max_rtvs as _,
        };

        let max_dsvs = 64; // TODO
        let dsv_pool = native::DescriptorCpuPool {
            heap: Self::create_descriptor_heap_impl(
                &mut device,
                winapi::D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                false,
                max_dsvs,
            ),
            offset: 0,
            size: 0,
            max_size: max_dsvs as _,
        };

        let max_srvs = 0x1000; // TODO
        let srv_pool = native::DescriptorCpuPool {
            heap: Self::create_descriptor_heap_impl(
                &mut device,
                winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                false,
                max_srvs,
            ),
            offset: 0,
            size: 0,
            max_size: max_srvs as _,
        };

        let max_uavs = 0x1000; // TODO
        let uav_pool = native::DescriptorCpuPool {
            heap: Self::create_descriptor_heap_impl(
                &mut device,
                winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                false,
                max_uavs,
            ),
            offset: 0,
            size: 0,
            max_size: max_uavs as _,
        };

        let max_samplers = 2048; // D3D12 doesn't allow more samplers for one heap.
        let sampler_pool = native::DescriptorCpuPool {
            heap: Self::create_descriptor_heap_impl(
                &mut device,
                winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
                false,
                max_samplers,
            ),
            offset: 0,
            size: 0,
            max_size: max_samplers as _,
        };

        let heap_srv_cbv_uav = Self::create_descriptor_heap_impl(
            &mut device,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            true,
            1_000_000, // maximum number of CBV/SRV/UAV descriptors in heap for Tier 1
        );

        let heap_sampler = Self::create_descriptor_heap_impl(
            &mut device,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            true,
            max_samplers,
        );

        Device {
            raw: device,
            features: Features { // TODO
                indirect_execution: true,
                draw_instanced: true,
                draw_instanced_base: true,
                draw_indexed_base: true,
                draw_indexed_instanced: true,
                draw_indexed_instanced_base_vertex: true,
                draw_indexed_instanced_base: true,
                instance_rate: false,
                vertex_base: false,
                srgb_color: false,
                constant_buffer: false,
                unordered_access_view: false,
                separate_blending_slots: false,
                copy_buffer: false,
                sampler_anisotropy: false,
                sampler_border_color: false,
                sampler_lod_bias: false,
                sampler_objects: false,
            },
            limits: Limits { // TODO
                max_texture_size: 0,
                max_patch_size: 0,
                max_viewports: 0,
                max_compute_group_count: [
                    winapi::D3D12_CS_THREAD_GROUP_MAX_X  as _,
                    winapi::D3D12_CS_THREAD_GROUP_MAX_Y  as _,
                    winapi::D3D12_CS_THREAD_GROUP_MAX_Z  as _,
                ],
                max_compute_group_size: [
                    winapi::D3D12_CS_THREAD_GROUP_MAX_THREADS_PER_GROUP as _,
                    1, //TODO
                    1, //TODO
                ],
                min_buffer_copy_offset_alignment: winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as _,
                min_buffer_copy_pitch_alignment: winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as _,
                min_uniform_buffer_offset_alignment: 256, // Required alignment for CBVs
            },
            private_caps: Capabilities {
                heterogeneous_resource_heaps: features.ResourceHeapTier != winapi::D3D12_RESOURCE_HEAP_TIER_1,
            },
            rtv_pool: Arc::new(Mutex::new(rtv_pool)),
            dsv_pool: Arc::new(Mutex::new(dsv_pool)),
            srv_pool: Arc::new(Mutex::new(srv_pool)),
            uav_pool: Arc::new(Mutex::new(uav_pool)),
            sampler_pool: Arc::new(Mutex::new(sampler_pool)),
            heap_srv_cbv_uav: Arc::new(Mutex::new(heap_srv_cbv_uav)),
            heap_sampler: Arc::new(Mutex::new(heap_sampler)),
            events: Vec::new(),
        }
    }
}

pub struct Instance {
    pub(crate) factory: ComPtr<winapi::IDXGIFactory4>,
}

impl Instance {
    pub fn create(_: &str, _: u32) -> Instance {
        #[cfg(debug_assertions)]
        {
            // Enable debug layer
            let mut debug_controller: *mut winapi::ID3D12Debug = ptr::null_mut();
            let hr = unsafe {
                d3d12::D3D12GetDebugInterface(
                    &dxguid::IID_ID3D12Debug,
                    &mut debug_controller as *mut *mut _ as *mut *mut c_void)
            };

            if winapi::SUCCEEDED(hr) {
                unsafe { (*debug_controller).EnableDebugLayer() };
            }

            unsafe { (*debug_controller).Release(); }
        }

        // Create DXGI factory
        let mut dxgi_factory: *mut winapi::IDXGIFactory4 = ptr::null_mut();

        let hr = unsafe {
            dxgi::CreateDXGIFactory2(
                winapi::DXGI_CREATE_FACTORY_DEBUG,
                &dxguid::IID_IDXGIFactory4,
                &mut dxgi_factory as *mut *mut _ as *mut *mut c_void)
        };

        if !winapi::SUCCEEDED(hr) {
            error!("Failed on dxgi factory creation: {:?}", hr);
        }

        Instance {
            factory: unsafe { ComPtr::new(dxgi_factory) },
        }
    }
}

impl core::Instance<Backend> for Instance {
    fn enumerate_adapters(&self) -> Vec<Adapter> {
        // Enumerate adapters
        let mut cur_index = 0;
        let mut devices = Vec::new();
        loop {
            let mut adapter = {
                let mut adapter: *mut winapi::IDXGIAdapter1 = ptr::null_mut();
                let hr = unsafe {
                    self.factory.as_mut().EnumAdapters1(
                        cur_index,
                        &mut adapter as *mut *mut _)
                };

                if hr == winapi::DXGI_ERROR_NOT_FOUND {
                    break;
                }

                unsafe { ComPtr::new(adapter as *mut winapi::IDXGIAdapter2) }
            };

            // Check for D3D12 support
            let hr = unsafe {
                d3d12::D3D12CreateDevice(
                    adapter.as_mut() as *mut _ as *mut winapi::IUnknown,
                    winapi::D3D_FEATURE_LEVEL_11_0, // TODO: correct feature level?
                    &dxguid::IID_ID3D12Device,
                    ptr::null_mut(),
                )
            };

            if winapi::SUCCEEDED(hr) {
                // We have found a possible adapter
                // acquire the device information
                let mut desc: winapi::DXGI_ADAPTER_DESC2 = unsafe { std::mem::uninitialized() };
                unsafe { adapter.GetDesc2(&mut desc); }

                let device_name = {
                    let len = desc.Description.iter().take_while(|&&c| c != 0).count();
                    let name = <OsString as OsStringExt>::from_wide(&desc.Description[..len]);
                    name.to_string_lossy().into_owned()
                };

                let info = core::AdapterInfo {
                    name: device_name,
                    vendor: desc.VendorId as usize,
                    device: desc.DeviceId as usize,
                    software_rendering: false, // TODO: check for WARP adapter (software rasterizer)?
                };

                devices.push(
                    Adapter {
                        adapter: adapter,
                        info: info,
                        queue_families: vec![(QueueFamily, QueueType::General)], // TODO:
                    });
            }

            cur_index += 1;
        }
        devices
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Adapter = Adapter;
    type Device = Device;

    type Surface = window::Surface;
    type Swapchain = window::Swapchain;

    type CommandQueue = CommandQueue;
    type CommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;
    type QueueFamily = QueueFamily;

    type Memory = native::Memory;
    type CommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::Framebuffer;

    type UnboundBuffer = device::UnboundBuffer;
    type Buffer = native::Buffer;
    type BufferView = native::BufferView;
    type UnboundImage = device::UnboundImage;
    type Image = native::Image;
    type ImageView = native::ImageView;
    type Sampler = native::Sampler;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
}
