use Device;
use descriptors::DualHandle;
use range_alloc::RangeAllocator;
use std::ops::Range;
use std::sync::Mutex;
use winapi::um::d3d12;
use wio::com::ComPtr;

/// Free-list heap allocator for GPU CBV/SRV/UAV descriptors.
pub struct CbvSrvUavGpuHeap {
    handle_size: usize,
    start: DualHandle,
    free_list: Mutex<RangeAllocator<u64>>,
    raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl CbvSrvUavGpuHeap {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, size: usize) -> Self {
        let ty = d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV;
        let heap = Device::create_descriptor_heap(device, ty, true, size);
        let handle_size = unsafe { device.GetDescriptorHandleIncrementSize(ty) as usize };
        let start = DualHandle {
            cpu: unsafe { heap.GetCPUDescriptorHandleForHeapStart() },
            gpu: unsafe { heap.GetGPUDescriptorHandleForHeapStart() },
        };

        CbvSrvUavGpuHeap {
            handle_size,
            start,
            free_list: Mutex::new(RangeAllocator::new(0..size as _)),
            raw: heap,
        }
    }

    pub fn allocate(&self, num: usize) -> Option<Range<u64>> {
        self.free_list
            .lock()
            .unwrap()
            .allocate_range(num as _)
    }

    pub fn free(&self, range: Range<u64>) {
        self.free_list
            .lock()
            .unwrap()
            .free_range(range);
    }

    pub fn as_raw(&self) -> *mut d3d12::ID3D12DescriptorHeap {
        self.raw.as_raw()
    }

    pub fn at(&self, idx: usize) -> DualHandle {
        DualHandle {
            cpu: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: self.start.cpu.ptr + self.handle_size * idx,
            },
            gpu: d3d12::D3D12_GPU_DESCRIPTOR_HANDLE {
                ptr: self.start.gpu.ptr + (self.handle_size * idx) as u64,
            },
        }
    }

    pub fn handle_size(&self) -> usize {
        self.handle_size
    }
}

