use Device;
use descriptors::DualHandle;
use range_alloc::RangeAllocator;
use std::ops::Range;
use std::sync::Mutex;
use winapi::um::d3d12;
use wio::com::ComPtr;

// Hidden GPU heap
struct Heap {
    start: DualHandle,
    free_list: RangeAllocator<u64>,
    _raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

/// Free-list heap allocator for managed GPU descriptors.
///
/// Due to D3D12 heap size limitations (e.g 2048 for sampelrs) we use an additional
/// CPU heap.
///
/// Strategy:
///  * At descriptor updates (write/copy) the descriptors will be written into the
///    the memory allocated by the CPU  heap.
///  * During descriptor set binding an optimal GPU heap will be chosen and if needed the
///    descriptor sets copied from the CPU  heap into the GPU heap.
pub struct ManagedHeap {
    start: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    handle_size: usize,
    free_list: Mutex<RangeAllocator<u64>>,
    gpu_heaps: Vec<Heap>,
    device: ComPtr<d3d12::ID3D12Device>,
    _raw_cpu: ComPtr<d3d12::ID3D12DescriptorHeap>,
    ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
}

impl ManagedHeap {
    pub fn new(
        device: &ComPtr<d3d12::ID3D12Device>,
        ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
        size: usize,
    ) -> Self {
        let cpu_heap = Device::create_descriptor_heap(device, ty, false, size);
        let handle_size = unsafe { device.GetDescriptorHandleIncrementSize(ty) as usize };
        let start = unsafe { cpu_heap.GetCPUDescriptorHandleForHeapStart() };

        ManagedHeap {
            start,
            handle_size,
            free_list: Mutex::new(RangeAllocator::new(0..size as _)),
            gpu_heaps: Vec::new(),
            device: device.clone(),
            _raw_cpu: cpu_heap,
            ty,
        }
    }

    pub fn allocate(&self, num: usize) -> Option<Range<u64>> {
        self.free_list.lock().unwrap().allocate_range(num as _)
    }

    pub fn free(&self, range: Range<u64>) {
        self.free_list.lock().unwrap().free_range(range);
    }

    fn create_gpu_heap(&mut self) {
        let num_samplers = 2_048; // Sampler heap size limit of D3D12
        let heap = Device::create_descriptor_heap(
            &self.device,
            d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            true,
            num_samplers,
        );
        let start = DualHandle {
            cpu: unsafe { heap.GetCPUDescriptorHandleForHeapStart() },
            gpu: unsafe { heap.GetGPUDescriptorHandleForHeapStart() },
        };

        self.gpu_heaps.push(Heap {
            start,
            free_list: RangeAllocator::new(0..num_samplers as _),
            _raw: heap,
        });
    }

    pub fn at(&self, idx: usize) -> DualHandle {
        DualHandle {
            cpu: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: self.start.ptr + self.handle_size * idx,
            },
            gpu: d3d12::D3D12_GPU_DESCRIPTOR_HANDLE { ptr: 0 },
        }
    }

    pub fn handle_size(&self) -> usize {
        self.handle_size
    }
}
