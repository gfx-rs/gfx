use Device;
use descriptors::DualHandle;
use range_alloc::RangeAllocator;
use winapi::um::d3d12;
use wio::com::ComPtr;

/// Free-list heap allocator for GPU CBV/SRV/UAV descriptors.
pub struct CbvSrvUavGpuHeap {
    handle_size: usize,
    start: DualHandle,
    free_list: RangeAllocator<u64>,
    _raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl CbvSrvUavGpuHeap {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, size: usize) -> Self {
        let heap = Device::create_descriptor_heap(
            device,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            true,
            size,
        );
        let handle_size = unsafe { device.GetDescriptorHandleIncrementSize(ty) as usize };
        let start = DualHandle {
            cpu: unsafe { heap.GetCPUDescriptorHandleForHeapStart() },
            gpu: unsafe { heap.GetGPUDescriptorHandleForHeapStart() },
        };

        CbvSrvUavGpuHeap {
            handle_size,
            start,
            free_list: RangeAllocator::new(0..size as _),
            _raw: unsafe { ComPtr::from_raw(heap) },
        }
    }

    pub fn allocate(&mut self, num: usize) -> Option<Range> {
        self.free_list.allocate_range(num as _)
    }

    pub fn free(&mut self, range: Range) {
        self.free_list.free_range(range);
    }
}

