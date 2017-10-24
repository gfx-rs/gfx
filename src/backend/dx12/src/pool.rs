use wio::com::ComPtr;
use dxguid;
use std::ptr;
use std::os::raw::c_void;
use winapi;

use hal::pool;
use command::CommandBuffer;
use Backend;

pub struct RawCommandPool {
    pub(crate) inner: ComPtr<winapi::ID3D12CommandAllocator>,
    pub(crate) device: ComPtr<winapi::ID3D12Device>,
    pub(crate) list_type: winapi::D3D12_COMMAND_LIST_TYPE,
}

impl RawCommandPool {
    fn create_command_list(&mut self) -> ComPtr<winapi::ID3D12GraphicsCommandList> {
        // allocate command lists
        let mut command_list = {
            let mut command_list: *mut winapi::ID3D12GraphicsCommandList = ptr::null_mut();
            let hr = unsafe {
                self.device.CreateCommandList(
                    0, // single gpu only atm
                    self.list_type,
                    self.inner.as_mut() as *mut _,
                    ptr::null_mut(),
                    &dxguid::IID_ID3D12GraphicsCommandList,
                    &mut command_list as *mut *mut _ as *mut *mut c_void,
                )
            };

            // TODO: error handling
            if !winapi::SUCCEEDED(hr) {
                error!("error on command list creation: {:x}", hr);
            }

            unsafe { ComPtr::new(command_list) }
        };

        // Close command list as they are initiated as recording.
        // But only one command list can be recording for each allocator
        unsafe { command_list.Close(); }

        command_list
    }
}

unsafe impl Send for RawCommandPool { }

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unsafe { self.inner.Reset(); }
    }

    fn allocate(&mut self, num: usize) -> Vec<CommandBuffer> {
        (0..num)
            .map(|_| CommandBuffer::new(self.create_command_list(), self.inner.clone()))
            .collect()
    }

    unsafe fn free(&mut self, _cbufs: Vec<CommandBuffer>) {
        // Just let the command buffers drop
    }
}

pub struct SubpassCommandPool;
impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {}
