
use winapi::um::d3d12;
use wio::com::ComPtr;

// Linear stack allocator for CPU descriptor heaps.
pub struct HeapLinear {
    handle_size: usize,
    num: usize,
    size: usize,
    start: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl HeapLinear {
    pub fn new(
        device: &ComPtr<d3d12::ID3D12Device>,
        ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
        size: usize,
    ) -> Self {
        unimplemented!()
    }

    pub fn alloc_handle(&mut self) -> d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
        assert!(!self.full());

        let slot = self.num;
        self.num += 1;

        d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.start.ptr + self.handle_size * slot,
        }
    }

    pub fn full(&self) -> bool {
        self.num < self.size
    }

    pub fn clear(&mut self) {
        self.num = 0;
    }
}

// Fixed-size (64) free-list allocator for CPU descriptors.
pub struct Heap {
    occupancy: u64,
    handle_size: usize,
    start: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl Heap {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE) -> Self {
        unimplemented!()
    }

    pub fn alloc_handle(&mut self) -> d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
        // Find first free slot
        let slot = (0..64)
            .position(|i| self.occupancy & (1 << i) == 0)
            .expect("Descriptor heap is full");
        self.occupancy |= 1 << slot;

        d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.start.ptr + self.handle_size * slot,
        }
    }

    pub fn full(&self) -> bool {
        self.occupancy == !0
    }
}

pub struct DescriptorCpuPool {
    device: ComPtr<d3d12::ID3D12Device>,
    ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
    heaps: Vec<Heap>,
    handle_size: usize,
}

impl DescriptorCpuPool {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE) -> Self {
        unimplemented!()
    }

    pub fn alloc_handle(&mut self) -> d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
        unimplemented!()
    }
}
