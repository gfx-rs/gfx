use Device;
use descriptors::DualHandle;
use range_alloc::RangeAllocator;
use winapi::um::d3d12;
use wio::com::ComPtr;

// Hidden GPU sampler heap
struct Heap {
    start: DualHandle,
    free_list: RangeAllocator<u64>,
    _raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

/// Free-list heap allocator for GPU sampler descriptors.
///
/// Due to D3D12 sampler heap size limtations (max 2048) we use an additional
/// CPU sampler heap.
///
/// Strategy:
///  * At descriptor updates (write/copy) the descriptors will be written into the
///    the memory allocated by the CPU sampler heap.
///  * During descriptor set binding an optimal GPU sampler heap will be chosen and if needed the
///    descriptor sets copied from the CPU sampler heap into the GPU heap.
pub struct SamplerGpuHeap {
    start: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    handle_size: usize,
    free_list: RangeAllocator<u64>,
    gpu_heaps: Vec<Heap>,
    device: ComPtr<d3d12::ID3D12Device>,
    _raw_cpu: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl SamplerGpuHeap {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, size: usize) -> Self {
        let cpu_heap = Device::create_descriptor_heap(
            device,
            D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            false,
            size,
        );
        let handle_size = unsafe {
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER) as usize
        };
        let start = unsafe { cpu_heap.GetCPUDescriptorHandleForHeapStart() };

        SamplerGpuHeap {
            start,
            handle_size,
            free_list: RangeAllocator::new(0 .. size as _),
            gpu_heaps: Vec::new(),
            device: device.clone(),
            _raw_cpu: cpu_heap,
        }
    }

    pub fn allocate(&mut self, num: usize) -> Option<Range> {
        self.free_list.allocate_range(num as _)
    }

    pub fn free(&mut self, range: Range) {
        self.free_list.free_range(range);
    }

    fn create_gpu_heap(&mut self) {
        let heap = Device::create_descriptor_heap(
            &self.device,
            D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            true,
            2_048, // Sampler heap size limit of D3D12
        );
        let start = DualHandle {
            cpu: unsafe { heap.GetCPUDescriptorHandleForHeapStart() },
            gpu: unsafe { heap.GetGPUDescriptorHandleForHeapStart() },
        };

        self.gpu_heaps.push(
            Heap {
                start,
                free_list: RangeAllocator::new(0 .. size as _),
                _raw: heap,
            }
        );
    }
}
