#[macro_use]
extern crate bitflags;
extern crate d3d12;
extern crate d3dcompiler;
extern crate dxguid;
extern crate dxgi;
extern crate gfx_hal as hal;
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

use hal::{memory, Features, Limits, QueueType};
use wio::com::ComPtr;

use std::{mem, ptr};
use std::os::raw::c_void;
use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;
use std::sync::Mutex;

pub(crate) struct HeapProperties {
    pub page_property: winapi::D3D12_CPU_PAGE_PROPERTY,
    pub memory_pool: winapi::D3D12_MEMORY_POOL,
}

// https://msdn.microsoft.com/de-de/library/windows/desktop/dn788678(v=vs.85).aspx
static HEAPS_NUMA: &'static [HeapProperties] = &[
    // DEFAULT
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
        memory_pool: winapi::D3D12_MEMORY_POOL_L1,
    },
    // UPLOAD
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,

    },
    // READBACK
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
];

static HEAPS_UMA: &'static [HeapProperties] = &[
    // DEFAULT
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
    // UPLOAD
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
    // READBACK
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
];

static HEAPS_CCUMA: &'static [HeapProperties] = &[
    // DEFAULT
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
    // UPLOAD
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
    //READBACK
    HeapProperties {
        page_property: winapi::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: winapi::D3D12_MEMORY_POOL_L0,
    },
];

#[derive(Debug)]
pub struct QueueFamily(QueueType);
const MAX_QUEUES: usize = 16; // infinite, to be fair

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType { self.0 }
    fn max_queues(&self) -> usize { MAX_QUEUES }
}

impl QueueFamily {
    fn native_type(&self) -> winapi::D3D12_COMMAND_LIST_TYPE {
        match self.0 {
            QueueType::General | QueueType::Graphics => winapi::D3D12_COMMAND_LIST_TYPE_DIRECT,
            QueueType::Compute => winapi::D3D12_COMMAND_LIST_TYPE_COMPUTE,
            QueueType::Transfer => winapi::D3D12_COMMAND_LIST_TYPE_COPY,
        }
    }
}

pub struct PhysicalDevice {
    adapter: ComPtr<winapi::IDXGIAdapter2>,
    factory: ComPtr<winapi::IDXGIFactory4>,
}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(self, families: Vec<(QueueFamily, usize)>) -> hal::Gpu<Backend> {
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
        let mut device = Device::new(unsafe { ComPtr::new(device_raw) });

        // Get the IDXGIAdapter3 from the created device to query video memory information.
        let mut adapter_id = unsafe { mem::uninitialized() };
        unsafe { device.raw.GetAdapterLuid(&mut adapter_id); }

        let mut adapter = {
            let mut adapter: *mut winapi::IDXGIAdapter3 = ptr::null_mut();
            unsafe {
                assert_eq!(winapi::S_OK, self.factory.as_mut().EnumAdapterByLuid(
                    adapter_id,
                    &dxguid::IID_IDXGIAdapter3,
                    &mut adapter as *mut *mut _ as *mut *mut _,
                ));
                ComPtr::new(adapter)
            }
        };

        let queue_groups = families
            .into_iter()
            .map(|(family, count)| {
                let queue_desc = winapi::D3D12_COMMAND_QUEUE_DESC {
                    Type: family.native_type(),
                    Priority: 0,
                    Flags: winapi::D3D12_COMMAND_QUEUE_FLAG_NONE,
                    NodeMask: 0,
                };
                let mut group = hal::queue::RawQueueGroup::new(family);

                for _ in 0 .. count {
                    let mut queue = ptr::null_mut();
                    let hr = unsafe {
                        device.raw.CreateCommandQueue(
                            &queue_desc,
                            &dxguid::IID_ID3D12CommandQueue,
                            &mut queue as *mut *mut _ as *mut *mut c_void,
                        )
                    };

                    if winapi::SUCCEEDED(hr) {
                        group.add_queue(CommandQueue {
                            raw: unsafe { ComPtr::new(queue) },
                            device: device.raw.clone(),
                        });
                    } else {
                        error!("error on queue creation: {:x}", hr);
                    }
                }

                group
            })
            .collect();

        // https://msdn.microsoft.com/en-us/library/windows/desktop/dn788678(v=vs.85).aspx
        let base_memory_types = match device.private_caps.memory_architecture {
            MemoryArchitecture::NUMA => [
                // DEFAULT
                hal::MemoryType {
                    id: 0,
                    properties: memory::DEVICE_LOCAL,
                    heap_index: 0,
                },
                // UPLOAD
                hal::MemoryType {
                    id: 1,
                    properties: memory::CPU_VISIBLE | memory::COHERENT,
                    heap_index: 1,
                },
                // READBACK
                hal::MemoryType {
                    id: 2,
                    properties: memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                    heap_index: 1,
                },
            ],
            MemoryArchitecture::UMA => [
                // DEFAULT
                hal::MemoryType {
                    id: 0,
                    properties: memory::DEVICE_LOCAL,
                    heap_index: 0,
                },
                // UPLOAD
                hal::MemoryType {
                    id: 1,
                    properties: memory::DEVICE_LOCAL | memory::CPU_VISIBLE | memory::COHERENT,
                    heap_index: 0,
                },
                // READBACK
                hal::MemoryType {
                    id: 2,
                    properties: memory::DEVICE_LOCAL | memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                    heap_index: 0,
                },
            ],
            MemoryArchitecture::CacheCoherentUMA => [
                // DEFAULT
                hal::MemoryType {
                    id: 0,
                    properties: memory::DEVICE_LOCAL,
                    heap_index: 0,
                },
                // UPLOAD
                hal::MemoryType {
                    id: 1,
                    properties: memory::DEVICE_LOCAL | memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                    heap_index: 0,
                },
                // READBACK
                hal::MemoryType {
                    id: 2,
                    properties: memory::DEVICE_LOCAL | memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                    heap_index: 0,
                },
            ],
        };

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
                    .map(|&mt| hal::MemoryType {
                        id: mt.id + (tt << 2),
                        .. mt
                    })
                );
            }
            types
        };

        let memory_heaps = {
            let mut query_memory = |segment: winapi::DXGI_MEMORY_SEGMENT_GROUP| unsafe {
                let mut mem_info: winapi::DXGI_QUERY_VIDEO_MEMORY_INFO = mem::uninitialized();
                assert_eq!(winapi::S_OK, adapter.QueryVideoMemoryInfo(
                    0,
                    segment,
                    &mut mem_info,
                ));
                mem_info.Budget
            };

            let local = query_memory(winapi::DXGI_MEMORY_SEGMENT_GROUP_LOCAL);
            match device.private_caps.memory_architecture {
                MemoryArchitecture::NUMA => {
                    let non_local = query_memory(winapi::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL);
                    vec![local, non_local]
                },
                _ => vec![local],
            }
        };

        hal::Gpu {
            device,
            queue_groups,
            memory_types,
            memory_heaps,
        }
    }
}

pub struct CommandQueue {
    pub(crate) raw: ComPtr<winapi::ID3D12CommandQueue>,
    device: ComPtr<winapi::ID3D12Device>,
}
unsafe impl Send for CommandQueue {} //blocked by ComPtr

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(
        &mut self,
        submission: hal::queue::RawSubmission<Backend>,
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

#[derive(Debug)]
enum MemoryArchitecture {
    NUMA,
    UMA,
    CacheCoherentUMA,
}

#[derive(Debug)]
pub struct Capabilities {
    heterogeneous_resource_heaps: bool,
    memory_architecture: MemoryArchitecture,
}

pub struct Device {
    raw: ComPtr<winapi::ID3D12Device>,
    features: hal::Features,
    limits: hal::Limits,
    private_caps: Capabilities,
    heap_properties: &'static [HeapProperties],
    // CPU only pools
    rtv_pool: Mutex<native::DescriptorCpuPool>,
    dsv_pool: Mutex<native::DescriptorCpuPool>,
    srv_pool: Mutex<native::DescriptorCpuPool>,
    uav_pool: Mutex<native::DescriptorCpuPool>,
    sampler_pool: Mutex<native::DescriptorCpuPool>,
    // CPU/GPU descriptor heaps
    heap_srv_cbv_uav: Mutex<native::DescriptorHeap>,
    heap_sampler: Mutex<native::DescriptorHeap>,
    events: Mutex<Vec<winapi::HANDLE>>,
}
unsafe impl Send for Device {} //blocked by ComPtr
unsafe impl Sync for Device {} //blocked by ComPtr

impl Device {
    fn new(mut device: ComPtr<winapi::ID3D12Device>) -> Self {
        let mut features: winapi::D3D12_FEATURE_DATA_D3D12_OPTIONS = unsafe { mem::zeroed() };
        assert_eq!(winapi::S_OK, unsafe {
            device.CheckFeatureSupport(winapi::D3D12_FEATURE_D3D12_OPTIONS,
                &mut features as *mut _ as *mut _,
                mem::size_of::<winapi::D3D12_FEATURE_DATA_D3D12_OPTIONS>() as _)
        });

        let mut features_architecture: winapi::D3D12_FEATURE_DATA_ARCHITECTURE = unsafe { mem::zeroed() };
        assert_eq!(winapi::S_OK, unsafe {
            device.CheckFeatureSupport(winapi::D3D12_FEATURE_ARCHITECTURE,
                &mut features_architecture as *mut _ as *mut _,
                mem::size_of::<winapi::D3D12_FEATURE_DATA_ARCHITECTURE>() as _)
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

        let uma = features_architecture.UMA == winapi::TRUE;
        let cc_uma = features_architecture.CacheCoherentUMA == winapi::TRUE;

        let (memory_architecture, heap_properties) = match (uma, cc_uma) {
            (true, true)  => (MemoryArchitecture::CacheCoherentUMA, HEAPS_CCUMA),
            (true, false) => (MemoryArchitecture::UMA, HEAPS_UMA),
            (false, _)            => (MemoryArchitecture::NUMA, HEAPS_NUMA),
        };

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
                memory_architecture,
            },
            heap_properties,
            rtv_pool: Mutex::new(rtv_pool),
            dsv_pool: Mutex::new(dsv_pool),
            srv_pool: Mutex::new(srv_pool),
            uav_pool: Mutex::new(uav_pool),
            sampler_pool: Mutex::new(sampler_pool),
            heap_srv_cbv_uav: Mutex::new(heap_srv_cbv_uav),
            heap_sampler: Mutex::new(heap_sampler),
            events: Mutex::new(Vec::new()),
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

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        // Enumerate adapters
        let mut cur_index = 0;
        let mut adapters = Vec::new();
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

                let info = hal::AdapterInfo {
                    name: device_name,
                    vendor: desc.VendorId as usize,
                    device: desc.DeviceId as usize,
                    software_rendering: false, // TODO: check for WARP adapter (software rasterizer)?
                };

                let physical_device = PhysicalDevice {
                    adapter,
                    factory: self.factory.clone(),
                };

                let queue_families = vec![
                    QueueFamily(QueueType::Transfer),
                    QueueFamily(QueueType::Compute),
                    QueueFamily(QueueType::General),
                ];

                adapters.push(hal::Adapter {
                    info,
                    physical_device,
                    queue_families
                });
            }

            cur_index += 1;
        }
        adapters
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = window::Surface;
    type Swapchain = window::Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = CommandQueue;
    type CommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;

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
