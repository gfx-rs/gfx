
mod cbv_srv_uav;
mod cpu;
mod sampler;

pub use cbv_srv_uav::CbvSrvUavGpuHeap;
pub use cpu::DescriptorCpuPool;
pub use sampler::SamplerGpuHeap;

#[derive(Copy, Clone)]
pub struct DualHandle {
    pub cpu: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    pub gpu: d3d12::D3D12_GPU_DESCRIPTOR_HANDLE,
}

impl DualHandle {
    pub fn offset(&self, offset: usize, handle_size: usize) -> DualHandle {
        DualHandle {
            cpu: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
                self.cpu.ptr + (handle_size * offset) as _,
            },
            gpu: d3d12::D3D12_GPU_DESCRIPTOR_HANDLE {
                self.gpu.ptr + (handle_size * offset) as _,
            },
        }
    }
}

// Linear stack allocator for descriptor heaps.
pub struct HeapLinear {
    handle_size: usize,
    num: usize,
    size: usize,
    start: DualHandle,
    raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl HeapLinear {
    pub fn new(
        device: &ComPtr<d3d12::ID3D12Device>,
        ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
        shader_visible: bool,
        size: usize,
    ) -> Self {
        let heap = Device::create_descriptor_heap(device, ty, shader_visible, size);
        let handle_size = unsafe { device.GetDescriptorHandleIncrementSize(ty) as usize };
        let start = DualHandle {
            cpu: unsafe { heap.GetCPUDescriptorHandleForHeapStart() },
            gpu: unsafe { heap.GetGPUDescriptorHandleForHeapStart() },
        };

        HeapLinear {
            handle_size,
            num: 0,
            size,
            start,
            raw: heap,
        }
    }

    pub fn alloc_handle(&mut self) -> DualHandle {
        assert!(!self.is_full());

        let slot = self.num;
        self.num += 1;

        d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.start.ptr + self.handle_size * slot,
        }
    }

    pub fn is_full(&self) -> bool {
        self.num >= self.size
    }

    pub fn clear(&mut self) {
        self.num = 0;
    }

    pub fn into_raw(self) -> ComPtr<d3d12::ID3D12DescriptorHeap> {
        self.raw
    }
}
