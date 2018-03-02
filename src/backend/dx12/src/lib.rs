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

mod command;
mod conv;
mod device;
mod format;
mod free_list;
mod native;
mod pool;
mod root_constants;
mod shade;
mod window;

use hal::{error, format as f, memory, Features, Limits, QueueType};
use hal::queue::{QueueFamily as HalQueueFamily, QueueFamilyId, Queues};

use winapi::shared::{dxgi, dxgi1_2, dxgi1_3, dxgi1_4, winerror};
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::um::{d3d12, d3d12sdklayers, d3dcommon, handleapi, synchapi, winbase, winnt};
use wio::com::ComPtr;

use std::{mem, ptr};
use std::borrow::{Borrow, BorrowMut};
use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

pub(crate) struct HeapProperties {
    pub page_property: d3d12::D3D12_CPU_PAGE_PROPERTY,
    pub memory_pool: d3d12::D3D12_MEMORY_POOL,
}

// https://msdn.microsoft.com/de-de/library/windows/desktop/dn770377(v=vs.85).aspx
// Only 16 input slots allowed.
const MAX_VERTEX_BUFFERS: usize = 16;

const NUM_HEAP_PROPERTIES: usize = 3;

// Memory types are grouped according to the supported resources.
// Grouping is done to circumvent the limitations of heap tier 1 devices.
// Devices with Tier 1 will expose `BuffersOnl`, `ImageOnly` and `TargetOnly`.
// Devices with Tier 2 or higher will only expose `Universal`.
enum MemoryGroup {
    Universal = 0,
    BufferOnly,
    ImageOnly,
    TargetOnly,

    NumGroups,
}

// https://msdn.microsoft.com/de-de/library/windows/desktop/dn788678(v=vs.85).aspx
static HEAPS_NUMA: [HeapProperties; NUM_HEAP_PROPERTIES] = [
    // DEFAULT
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L1,
    },
    // UPLOAD
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,

    },
    // READBACK
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
];

static HEAPS_UMA: [HeapProperties; NUM_HEAP_PROPERTIES] = [
    // DEFAULT
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
    // UPLOAD
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
    // READBACK
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
];

static HEAPS_CCUMA: [HeapProperties; NUM_HEAP_PROPERTIES] = [
    // DEFAULT
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
    // UPLOAD
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
    //READBACK
    HeapProperties {
        page_property: d3d12::D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
        memory_pool: d3d12::D3D12_MEMORY_POOL_L0,
    },
];

#[derive(Debug, Copy, Clone)]
pub enum QueueFamily {
    // Specially marked present queue.
    // It's basically a normal 3D queue but D3D12 swapchain creation requires an
    // associated queue, which we don't know on `create_swapchain`.
    Present,
    Normal(QueueType),
}

const MAX_QUEUES: usize = 16; // infinite, to be fair

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType {
        match *self {
            QueueFamily::Present => QueueType::General,
            QueueFamily::Normal(ty) => ty,
        }
    }
    fn max_queues(&self) -> usize {
        match *self {
            QueueFamily::Present => 1,
            QueueFamily::Normal(_) => MAX_QUEUES,
        }
    }
    fn id(&self) -> QueueFamilyId {
        // This must match the order exposed by `QUEUE_FAMILIES`
        QueueFamilyId(match *self {
            QueueFamily::Present => 0,
            QueueFamily::Normal(QueueType::General) => 1,
            QueueFamily::Normal(QueueType::Compute) => 2,
            QueueFamily::Normal(QueueType::Transfer) => 3,
            _ => unreachable!(),
        })
    }
}

impl QueueFamily {
    fn native_type(&self) -> d3d12::D3D12_COMMAND_LIST_TYPE {
        use hal::QueueFamily;
        let queue_type = self.queue_type();
        match queue_type {
            QueueType::General | QueueType::Graphics => d3d12::D3D12_COMMAND_LIST_TYPE_DIRECT,
            QueueType::Compute => d3d12::D3D12_COMMAND_LIST_TYPE_COMPUTE,
            QueueType::Transfer => d3d12::D3D12_COMMAND_LIST_TYPE_COPY,
        }
    }
}

static QUEUE_FAMILIES: [QueueFamily; 4] = [
    QueueFamily::Present,
    QueueFamily::Normal(QueueType::General),
    QueueFamily::Normal(QueueType::Compute),
    QueueFamily::Normal(QueueType::Transfer),
];

pub struct PhysicalDevice {
    adapter: ComPtr<dxgi1_2::IDXGIAdapter2>,
    features: hal::Features,
    limits: hal::Limits,
    private_caps: Capabilities,
    heap_properties: &'static [HeapProperties; NUM_HEAP_PROPERTIES],
    memory_properties: hal::MemoryProperties,
    // Indicates that there is currently an active logical device.
    // Opening the same adapter multiple times will return the same D3D12Device again.
    is_open: Arc<Mutex<bool>>,
}

unsafe impl Send for PhysicalDevice { }
unsafe impl Sync for PhysicalDevice { }

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(
        &self, families: Vec<(&QueueFamily, Vec<hal::QueuePriority>)>
    ) -> Result<hal::Gpu<Backend>, error::DeviceCreationError> {
        let lock = self.is_open.try_lock();
        let mut open_guard = match lock {
            Ok(inner) => inner,
            Err(_) => return Err(error::DeviceCreationError::TooManyObjects),
        };

        // Create D3D12 device
        let device_raw = {
            let mut device_raw = ptr::null_mut();
            let hr = unsafe {
                d3d12::D3D12CreateDevice(
                    self.adapter.as_raw() as *mut _,
                    d3dcommon::D3D_FEATURE_LEVEL_11_0, // Minimum required feature level
                    &d3d12::IID_ID3D12Device,
                    &mut device_raw as *mut *mut _ as *mut *mut _,
                )
            };
            if !winerror::SUCCEEDED(hr) {
                error!("error on device creation: {:x}", hr);
            }

            unsafe { ComPtr::<d3d12::ID3D12Device>::from_raw(device_raw) }
        };

        // Always create the presentation queue in case we want to build a swapchain.
        let present_queue = {
            let queue_desc = d3d12::D3D12_COMMAND_QUEUE_DESC {
                Type: QueueFamily::Present.native_type(),
                Priority: 0,
                Flags: d3d12::D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };

            let mut queue = ptr::null_mut();
            let hr = unsafe {
                device_raw.CreateCommandQueue(
                    &queue_desc,
                    &d3d12::IID_ID3D12CommandQueue,
                    &mut queue as *mut *mut _ as *mut *mut _,
                )
            };

            if !winerror::SUCCEEDED(hr) {
                error!("error on queue creation: {:x}", hr);
            }

            unsafe { ComPtr::<d3d12::ID3D12CommandQueue>::from_raw(queue) }
        };

        let mut device = Device::new(
            device_raw,
            &self,
            present_queue,
        );

        let queue_groups = families
            .into_iter()
            .map(|(&family, priorities)| {
                let mut group = hal::backend::RawQueueGroup::new(family);

                let create_idle_event = || unsafe {
                    synchapi::CreateEventA(
                        ptr::null_mut(),
                        TRUE, // Want to manually reset in case multiple threads wait for idle
                        FALSE,
                        ptr::null(),
                    )
                };

                match family {
                    QueueFamily::Present => {
                        // Exactly **one** present queue!
                        // Number of queues need to be larger than 0 else it
                        // violates the specification.
                        let queue = CommandQueue {
                            raw: device.present_queue.clone(),
                            idle_fence: device.create_raw_fence(false),
                            idle_event: create_idle_event(),
                        };
                        device.append_queue(queue.clone());
                        group.add_queue(queue);
                    }
                    QueueFamily::Normal(_) => {
                        let queue_desc = d3d12::D3D12_COMMAND_QUEUE_DESC {
                            Type: family.native_type(),
                            Priority: 0,
                            Flags: d3d12::D3D12_COMMAND_QUEUE_FLAG_NONE,
                            NodeMask: 0,
                        };

                        for _ in 0 .. priorities.len() {
                            let mut queue = ptr::null_mut();
                            let hr = unsafe {
                                device.raw.CreateCommandQueue(
                                    &queue_desc,
                                    &d3d12::IID_ID3D12CommandQueue,
                                    &mut queue as *mut *mut _ as *mut *mut _,
                                )
                            };

                            if winerror::SUCCEEDED(hr) {
                                let queue = CommandQueue {
                                    raw: unsafe { ComPtr::from_raw(queue) },
                                    idle_fence: device.create_raw_fence(false),
                                    idle_event: create_idle_event(),
                                };
                                device.append_queue(queue.clone());
                                group.add_queue(queue);
                            } else {
                                error!("error on queue creation: {:x}", hr);
                            }
                        }
                    }
                }

                (family.id(), group)
            })
            .collect();

        *open_guard = true;

        Ok(hal::Gpu {
            device,
            queues: Queues::new(queue_groups),
        })
    }

    fn format_properties(&self, fmt: Option<f::Format>) -> f::Properties {
        let idx = fmt.map(|fmt| fmt as usize).unwrap_or(0);
        format::query_properties()[idx]
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        self.memory_properties.clone()
    }

    fn features(&self) -> Features { self.features }
    fn limits(&self) -> Limits { self.limits }
}

#[derive(Clone)]
pub struct CommandQueue {
    pub(crate) raw: ComPtr<d3d12::ID3D12CommandQueue>,
    idle_fence: *mut d3d12::ID3D12Fence,
    idle_event: winnt::HANDLE,
}

unsafe impl Send for CommandQueue {}
unsafe impl Sync for CommandQueue {}

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(
        &mut self,
        submission: hal::queue::RawSubmission<Backend, IC>,
        fence: Option<&native::Fence>,
    ) where
        IC: IntoIterator,
        IC::Item: Borrow<command::CommandBuffer>,
    {
        // Reset idle fence and event
        // That's safe here due to exclusive access to the queue
        (*self.idle_fence).Signal(0);
        synchapi::ResetEvent(self.idle_event);

        // TODO: semaphores
        let mut lists = submission
            .cmd_buffers
            .into_iter()
            .map(|buf| buf.borrow().as_raw_list())
            .collect::<Vec<_>>();
        self.raw.ExecuteCommandLists(lists.len() as _, lists.as_mut_ptr());

        if let Some(fence) = fence {
            assert_eq!(winerror::S_OK,
                self.raw.Signal(fence.raw.as_raw(), 1)
            );
        }
    }

    fn present<IS, IW>(&mut self, swapchains: IS, _wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<window::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<native::Semaphore>,
    {
        // TODO: semaphores
        for swapchain in swapchains {
            unsafe { swapchain.borrow().inner.Present(1, 0); }
        }
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unsafe {
            self.raw.Signal(self.idle_fence, 1);
            assert_eq!(winerror::S_OK, (*self.idle_fence).SetEventOnCompletion(1, self.idle_event));
            synchapi::WaitForSingleObject(self.idle_event, winbase::INFINITE);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum MemoryArchitecture {
    NUMA,
    UMA,
    CacheCoherentUMA,
}

#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    heterogeneous_resource_heaps: bool,
    memory_architecture: MemoryArchitecture,
}

#[derive(Clone)]
struct CmdSignatures {
    draw: ComPtr<d3d12::ID3D12CommandSignature>,
    draw_indexed: ComPtr<d3d12::ID3D12CommandSignature>,
    dispatch: ComPtr<d3d12::ID3D12CommandSignature>,
}

pub struct Device {
    raw: ComPtr<d3d12::ID3D12Device>,
    private_caps: Capabilities,
    heap_properties: &'static [HeapProperties],
    // CPU only pools
    rtv_pool: Mutex<native::DescriptorCpuPool>,
    dsv_pool: Mutex<native::DescriptorCpuPool>,
    srv_pool: Mutex<native::DescriptorCpuPool>,
    uav_pool: Mutex<native::DescriptorCpuPool>,
    sampler_pool: Mutex<native::DescriptorCpuPool>,
    descriptor_update_pools: Mutex<Vec<native::DescriptorCpuPool>>,
    // CPU/GPU descriptor heaps
    heap_srv_cbv_uav: Mutex<native::DescriptorHeap>,
    heap_sampler: Mutex<native::DescriptorHeap>,
    events: Mutex<Vec<winnt::HANDLE>>,
    signatures: CmdSignatures,
    // Present queue exposed by the `Present` queue family.
    // Required for swapchain creation. Only a single queue supports presentation.
    present_queue: ComPtr<d3d12::ID3D12CommandQueue>,
    // List of all queues created from this device, including present queue.
    // Needed for `wait_idle`.
    queues: Vec<CommandQueue>,
    // Indicates that there is currently an active device.
    open: Arc<Mutex<bool>>,
}
unsafe impl Send for Device {} //blocked by ComPtr
unsafe impl Sync for Device {} //blocked by ComPtr

impl Device {
    fn new(
        mut device: ComPtr<d3d12::ID3D12Device>,
        physical_device: &PhysicalDevice,
        present_queue: ComPtr<d3d12::ID3D12CommandQueue>,
    ) -> Self {
        // Allocate descriptor heaps
        let max_rtvs = 256; // TODO
        let rtv_pool = native::DescriptorCpuPool {
            heap: Self::create_descriptor_heap_impl(
                &mut device,
                d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
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
                d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
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
                d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
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
                d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
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
                d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
                false,
                max_samplers,
            ),
            offset: 0,
            size: 0,
            max_size: max_samplers as _,
        };

        let heap_srv_cbv_uav = Self::create_descriptor_heap_impl(
            &mut device,
            d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            true,
            1_000_000, // maximum number of CBV/SRV/UAV descriptors in heap for Tier 1
        );

        let heap_sampler = Self::create_descriptor_heap_impl(
            &mut device,
            d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            true,
            max_samplers,
        );

        let draw_signature = Self::create_command_signature(
            &mut device,
            device::CommandSignature::Draw,
        );

        let draw_indexed_signature = Self::create_command_signature(
            &mut device,
            device::CommandSignature::DrawIndexed,
        );

        let dispatch_signature = Self::create_command_signature(
            &mut device,
            device::CommandSignature::Dispatch,
        );

        Device {
            raw: device,
            private_caps: physical_device.private_caps,
            heap_properties: physical_device.heap_properties,
            rtv_pool: Mutex::new(rtv_pool),
            dsv_pool: Mutex::new(dsv_pool),
            srv_pool: Mutex::new(srv_pool),
            uav_pool: Mutex::new(uav_pool),
            sampler_pool: Mutex::new(sampler_pool),
            descriptor_update_pools: Mutex::new(Vec::new()),
            heap_srv_cbv_uav: Mutex::new(heap_srv_cbv_uav),
            heap_sampler: Mutex::new(heap_sampler),
            events: Mutex::new(Vec::new()),
            signatures: CmdSignatures {
                draw: draw_signature,
                draw_indexed: draw_indexed_signature,
                dispatch: dispatch_signature,
            },
            present_queue,
            queues: Vec::new(),
            open: physical_device.is_open.clone(),
        }
    }

    fn append_queue(&mut self, queue: CommandQueue) {
        self.queues.push(queue);
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        *self.open.lock().unwrap() = false;
        for queue in &mut self.queues {
            unsafe {
                (*queue.idle_fence).Release();
                handleapi::CloseHandle(queue.idle_event);
            }
        }
    }
}

pub struct Instance {
    pub(crate) factory: ComPtr<dxgi1_4::IDXGIFactory4>,
}

unsafe impl Send for Instance { }
unsafe impl Sync for Instance { }

impl Instance {
    pub fn create(_: &str, _: u32) -> Instance {
        #[cfg(debug_assertions)]
        {
            // Enable debug layer
            let mut debug_controller: *mut d3d12sdklayers::ID3D12Debug = ptr::null_mut();
            let hr = unsafe {
                d3d12::D3D12GetDebugInterface(
                    &d3d12sdklayers::IID_ID3D12Debug,
                    &mut debug_controller as *mut *mut _ as *mut *mut _)
            };

            if winerror::SUCCEEDED(hr) {
                unsafe { (*debug_controller).EnableDebugLayer() };
                unsafe { (*debug_controller).Release(); }
            }
        }

        // Create DXGI factory
        let mut dxgi_factory: *mut dxgi1_4::IDXGIFactory4 = ptr::null_mut();

        let hr = unsafe {
            dxgi1_3::CreateDXGIFactory2(
                dxgi1_3::DXGI_CREATE_FACTORY_DEBUG,
                &dxgi1_4::IID_IDXGIFactory4,
                &mut dxgi_factory as *mut *mut _ as *mut *mut _)
        };

        if !winerror::SUCCEEDED(hr) {
            error!("Failed on dxgi factory creation: {:?}", hr);
        }

        Instance {
            factory: unsafe { ComPtr::from_raw(dxgi_factory) },
        }
    }
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        use self::memory::Properties;

        // Enumerate adapters
        let mut cur_index = 0;
        let mut adapters = Vec::new();
        loop {
            let adapter = {
                let mut adapter: *mut dxgi::IDXGIAdapter1 = ptr::null_mut();
                let hr = unsafe {
                    self.factory.EnumAdapters1(
                        cur_index,
                        &mut adapter as *mut *mut _)
                };

                if hr == winerror::DXGI_ERROR_NOT_FOUND {
                    break;
                }

                unsafe { ComPtr::from_raw(adapter as *mut dxgi1_2::IDXGIAdapter2) }
            };

            cur_index += 1;

            // Check for D3D12 support
            // Create temporaty device to get physical device information
            let device = {
                let mut device = ptr::null_mut();
                let hr = unsafe {
                    d3d12::D3D12CreateDevice(
                        adapter.as_raw() as *mut _,
                        d3dcommon::D3D_FEATURE_LEVEL_11_0,
                        &d3d12::IID_ID3D12Device,
                        &mut device as *mut *mut _ as *mut *mut _,
                    )
                };
                if !winerror::SUCCEEDED(hr) {
                    continue;
                }

                unsafe { ComPtr::<d3d12::ID3D12Device>::from_raw(device) }
            };

            // We have found a possible adapter
            // acquire the device information
            let mut desc: dxgi1_2::DXGI_ADAPTER_DESC2 = unsafe { mem::zeroed() };
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

            let mut features: d3d12::D3D12_FEATURE_DATA_D3D12_OPTIONS = unsafe { mem::zeroed() };
            assert_eq!(winerror::S_OK, unsafe {
                device.CheckFeatureSupport(d3d12::D3D12_FEATURE_D3D12_OPTIONS,
                    &mut features as *mut _ as *mut _,
                    mem::size_of::<d3d12::D3D12_FEATURE_DATA_D3D12_OPTIONS>() as _)
            });

            let mut features_architecture: d3d12::D3D12_FEATURE_DATA_ARCHITECTURE = unsafe { mem::zeroed() };
            assert_eq!(winerror::S_OK, unsafe {
                device.CheckFeatureSupport(d3d12::D3D12_FEATURE_ARCHITECTURE,
                    &mut features_architecture as *mut _ as *mut _,
                    mem::size_of::<d3d12::D3D12_FEATURE_DATA_ARCHITECTURE>() as _)
            });

            let heterogeneous_resource_heaps = features.ResourceHeapTier != d3d12::D3D12_RESOURCE_HEAP_TIER_1;

            let uma = features_architecture.UMA == TRUE;
            let cc_uma = features_architecture.CacheCoherentUMA == TRUE;

            let (memory_architecture, heap_properties) = match (uma, cc_uma) {
                (true, true)  => (MemoryArchitecture::CacheCoherentUMA, &HEAPS_CCUMA),
                (true, false) => (MemoryArchitecture::UMA, &HEAPS_UMA),
                (false, _)    => (MemoryArchitecture::NUMA, &HEAPS_NUMA),
            };

            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn788678(v=vs.85).aspx
            let base_memory_types: [hal::MemoryType; NUM_HEAP_PROPERTIES] = match memory_architecture {
                MemoryArchitecture::NUMA => [
                    // DEFAULT
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL,
                        heap_index: 0,
                    },
                    // UPLOAD
                    hal::MemoryType {
                        properties: Properties::CPU_VISIBLE | Properties::COHERENT,
                        heap_index: 1,
                    },
                    // READBACK
                    hal::MemoryType {
                        properties: Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                        heap_index: 1,
                    },
                ],
                MemoryArchitecture::UMA => [
                    // DEFAULT
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL,
                        heap_index: 0,
                    },
                    // UPLOAD
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::COHERENT,
                        heap_index: 0,
                    },
                    // READBACK
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                        heap_index: 0,
                    },
                ],
                MemoryArchitecture::CacheCoherentUMA => [
                    // DEFAULT
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL,
                        heap_index: 0,
                    },
                    // UPLOAD
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                        heap_index: 0,
                    },
                    // READBACK
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                        heap_index: 0,
                    },
                ],
            };

            let memory_types = if heterogeneous_resource_heaps {
                base_memory_types.to_vec()
            } else {
                // We multiplicate the base memory types depending on the resource usage:
                //     0.. 3: Reserved for futures use
                //     4.. 6: Buffers
                //     7.. 9: Images
                //    10..12: Targets
                //
                // The supported memory types for a resource can be requested by asking for
                // the memory requirements. Memory type indices are encoded as bitflags.
                // `device::MEM_TYPE_MASK` (0b111) defines the bitmask for one base memory type group.
                // The corresponding shift masks (`device::MEM_TYPE_BUFFER_SHIFT`,
                // `device::MEM_TYPE_IMAGE_SHIFT`, `device::MEM_TYPE_TARGET_SHIFT`)
                // denote the usage group.
                let mut types = Vec::new();
                for i in 0 .. MemoryGroup::NumGroups as _ {
                    types.extend(base_memory_types
                        .iter()
                        .map(|mem_type| {
                            let mut ty = mem_type.clone();

                            // Images and Targets are not host visible as we can't create
                            // a corresponding buffer for mapping.
                            if i == MemoryGroup::ImageOnly as _ || i == MemoryGroup::TargetOnly as _ {
                                ty.properties.remove(Properties::CPU_VISIBLE);
                            }
                            ty
                        })
                    );
                }
                types
            };

            let memory_heaps = {
                // Get the IDXGIAdapter3 from the created device to query video memory information.
                let adapter_id = unsafe { device.GetAdapterLuid() };
                let adapter = {
                    let mut adapter: *mut dxgi1_4::IDXGIAdapter3 = ptr::null_mut();
                    unsafe {
                        assert_eq!(winerror::S_OK, self.factory.EnumAdapterByLuid(
                            adapter_id,
                            &dxgi1_4::IID_IDXGIAdapter3,
                            &mut adapter as *mut *mut _ as *mut *mut _,
                        ));
                        ComPtr::from_raw(adapter)
                    }
                };

                let query_memory = |segment: dxgi1_4::DXGI_MEMORY_SEGMENT_GROUP| unsafe {
                    let mut mem_info: dxgi1_4::DXGI_QUERY_VIDEO_MEMORY_INFO = mem::uninitialized();
                    assert_eq!(winerror::S_OK, adapter.QueryVideoMemoryInfo(
                        0,
                        segment,
                        &mut mem_info,
                    ));
                    mem_info.Budget
                };

                let local = query_memory(dxgi1_4::DXGI_MEMORY_SEGMENT_GROUP_LOCAL);
                match memory_architecture {
                    MemoryArchitecture::NUMA => {
                        let non_local = query_memory(dxgi1_4::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL);
                        vec![local, non_local]
                    },
                    _ => vec![local],
                }
            };

            let physical_device = PhysicalDevice {
                adapter,
                features:
                    // TODO: add more features, based on
                    // https://msdn.microsoft.com/de-de/library/windows/desktop/mt186615(v=vs.85).aspx
                    Features::IMAGE_CUBE_ARRAY |
                    Features::GEOMETRY_SHADER |
                    Features::TESSELLATION_SHADER |
                    //logic_op: false, // Optional on feature level 11_0
                    Features::MULTI_DRAW_INDIRECT |
                    Features::FORMAT_BC |
                    Features::INSTANCE_RATE,
                limits: Limits { // TODO
                    max_texture_size: 0,
                    max_patch_size: 0,
                    max_viewports: 0,
                    max_compute_group_count: [
                        d3d12::D3D12_CS_THREAD_GROUP_MAX_X,
                        d3d12::D3D12_CS_THREAD_GROUP_MAX_Y,
                        d3d12::D3D12_CS_THREAD_GROUP_MAX_Z,
                    ],
                    max_compute_group_size: [
                        d3d12::D3D12_CS_THREAD_GROUP_MAX_THREADS_PER_GROUP,
                        1, //TODO
                        1, //TODO
                    ],
                    min_buffer_copy_offset_alignment: d3d12::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as _,
                    min_buffer_copy_pitch_alignment: d3d12::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as _,
                    min_uniform_buffer_offset_alignment: 256, // Required alignment for CBVs
                },
                private_caps: Capabilities {
                    heterogeneous_resource_heaps,
                    memory_architecture,
                },
                heap_properties,
                memory_properties: hal::MemoryProperties {
                    memory_types,
                    memory_heaps,
                },
                is_open: Arc::new(Mutex::new(false)),
            };

            let queue_families = QUEUE_FAMILIES.to_vec();

            adapters.push(hal::Adapter {
                info,
                physical_device,
                queue_families,
            });
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

    type Memory = native::Memory;
    type CommandPool = pool::RawCommandPool;

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
    type QueryPool = native::QueryPool;
}
