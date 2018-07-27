
use std::collections::HashSet;
use std::ptr;
use winapi::Interface;
use winapi::um::d3d12;
use wio::com::ComPtr;

// Linear stack allocator for CPU descriptor heaps.
pub struct HeapLinear {
    handle_size: usize,
    num: usize,
    size: usize,
    start: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    _raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl HeapLinear {
    pub fn new(
        device: &ComPtr<d3d12::ID3D12Device>,
        ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
        size: usize,
    ) -> Self {
        let desc = d3d12::D3D12_DESCRIPTOR_HEAP_DESC {
            Type: ty,
            NumDescriptors: size as u32,
            Flags: d3d12::D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };

        let mut heap: *mut d3d12::ID3D12DescriptorHeap = ptr::null_mut();
        let handle_size = unsafe {
            device.CreateDescriptorHeap(
                &desc,
                &d3d12::ID3D12DescriptorHeap::uuidof(),
                &mut heap as *mut *mut _ as *mut *mut _,
            );
            device.GetDescriptorHandleIncrementSize(ty) as usize
        };

        let start = unsafe { (*heap).GetCPUDescriptorHandleForHeapStart() };

        HeapLinear {
            handle_size,
            num: 0,
            size,
            start,
            _raw: unsafe { ComPtr::from_raw(heap) },
        }
    }

    pub fn alloc_handle(&mut self) -> d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
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
}

const HEAP_SIZE_FIXED: usize = 64;

// Fixed-size free-list allocator for CPU descriptors.
struct Heap {
    // Bit flag representation of available handles in the heap.
    //
    //  0 - Occupied
    //  1 - free
    availability: u64,
    handle_size: usize,
    start: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    _raw: ComPtr<d3d12::ID3D12DescriptorHeap>,
}

impl Heap {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE) -> Self {
        let desc = d3d12::D3D12_DESCRIPTOR_HEAP_DESC {
            Type: ty,
            NumDescriptors: HEAP_SIZE_FIXED as _,
            Flags: d3d12::D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };

        let mut heap: *mut d3d12::ID3D12DescriptorHeap = ptr::null_mut();
        let handle_size = unsafe {
            device.CreateDescriptorHeap(
                &desc,
                &d3d12::ID3D12DescriptorHeap::uuidof(),
                &mut heap as *mut *mut _ as *mut *mut _,
            );
            device.GetDescriptorHandleIncrementSize(ty) as usize
        };
        let start = unsafe { (*heap).GetCPUDescriptorHandleForHeapStart() };

        Heap {
            handle_size,
            availability: !0, // all free!
            start,
            _raw: unsafe { ComPtr::from_raw(heap) },
        }
    }

    pub fn alloc_handle(&mut self) -> d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
        // Find first free slot.
        let slot = self.availability.trailing_zeros() as usize;
        assert!(slot < HEAP_SIZE_FIXED);
        // Set the slot as occupied.
        self.availability ^= 1 << slot;

        d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.start.ptr + self.handle_size * slot,
        }
    }

    pub fn is_full(&self) -> bool {
        self.availability == 0
    }
}

pub struct DescriptorCpuPool {
    device: ComPtr<d3d12::ID3D12Device>,
    ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
    heaps: Vec<Heap>,
    free_list: HashSet<usize>,
}

impl DescriptorCpuPool {
    pub fn new(device: &ComPtr<d3d12::ID3D12Device>, ty: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE) -> Self {
        DescriptorCpuPool {
            device: device.clone(),
            ty,
            heaps: Vec::new(),
            free_list: HashSet::new(),
        }
    }

    pub fn alloc_handle(&mut self) -> d3d12::D3D12_CPU_DESCRIPTOR_HANDLE {
        let heap_id = self
            .free_list
            .iter()
            .cloned()
            .next()
            .unwrap_or_else(|| {
                // Allocate a new heap
                let id = self.heaps.len();
                self.heaps.push(Heap::new(&self.device, self.ty));
                self.free_list.insert(id);
                id
            });

        let heap = &mut self.heaps[heap_id];
        let handle = heap.alloc_handle();
        if heap.is_full() {
            self.free_list.remove(&heap_id);
        }

        handle
    }

    // TODO: free handles
}
