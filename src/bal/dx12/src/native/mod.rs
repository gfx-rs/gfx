use std::ffi::CStr;
use winapi::um::d3dcommon;

mod com;
pub mod command_allocator;
pub mod command_list;
pub mod descriptor;
pub mod device;
pub mod pso;
pub mod query;
pub mod queue;
pub mod resource;
pub mod sync;

pub use self::com::WeakPtr;
pub use self::command_allocator::CommandAllocator;
pub use self::command_list::{CommandSignature, GraphicsCommandList};
pub use self::descriptor::{CpuDescriptor, DescriptorHeap, GpuDescriptor, RootSignature};
pub use self::device::Device;
pub use self::pso::{CachedPSO, PipelineState, Shader};
pub use self::query::QueryHeap;
pub use self::queue::CommandQueue;
pub use self::resource::{Heap, Resource};
pub use self::sync::Fence;

pub type Blob = com::WeakPtr<d3dcommon::ID3DBlob>;
pub type Error = com::WeakPtr<d3dcommon::ID3DBlob>;
impl Error {
    pub unsafe fn as_c_str(&self) -> &CStr {
        debug_assert!(!self.is_null());
        let data = self.GetBufferPointer();
        CStr::from_ptr(data as *const _ as *const _)
    }
}
