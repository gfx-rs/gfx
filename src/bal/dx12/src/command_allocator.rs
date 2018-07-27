//! Command Allocator

#[repr(transparent)]
pub struct CommandAllocator(ComPtr<d3d12::ID3D12CommandAlloator>);

impl CommandAllocator {
    pub fn reset(&self) {
        unsafe { self.0.Reset(); }
    }
}
