use wio::com::ComPtr;
use std::ptr;

use winapi::um::d3d12;
use winapi::shared::winerror::SUCCEEDED;

use hal::{pool, command};
use command::{CommandBuffer};
use {Backend, CmdSignatures};

pub struct RawCommandPool {
    pub(crate) inner: ComPtr<d3d12::ID3D12CommandAllocator>,
    pub(crate) device: ComPtr<d3d12::ID3D12Device>,
    pub(crate) list_type: d3d12::D3D12_COMMAND_LIST_TYPE,
    pub(crate) signatures: CmdSignatures,
}

impl RawCommandPool {
    fn create_command_list(&mut self) -> ComPtr<d3d12::ID3D12GraphicsCommandList> {
        // allocate command lists
        let command_list = {
            let mut command_list: *mut d3d12::ID3D12GraphicsCommandList = ptr::null_mut();
            let hr = unsafe {
                self.device.CreateCommandList(
                    0, // single gpu only atm
                    self.list_type,
                    self.inner.as_raw(),
                    ptr::null_mut(),
                    &d3d12::IID_ID3D12GraphicsCommandList,
                    &mut command_list as *mut *mut _ as *mut *mut _,
                )
            };

            // TODO: error handling
            if !SUCCEEDED(hr) {
                error!("error on command list creation: {:x}", hr);
            }

            unsafe { ComPtr::from_raw(command_list) }
        };

        // Close command list as they are initiated as recording.
        // But only one command list can be recording for each allocator
        unsafe { command_list.Close(); }

        command_list
    }
}

unsafe impl Send for RawCommandPool { }
unsafe impl Sync for RawCommandPool { }

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unsafe { self.inner.Reset(); }
    }

    fn allocate(
        &mut self, num: usize, level: command::RawLevel
    ) -> Vec<CommandBuffer> { // TODO: Implement secondary buffers
        assert_eq!(level, command::RawLevel::Primary);
        (0..num)
            .map(|_| CommandBuffer::new(
                self.create_command_list(),
                self.inner.clone(),
                self.signatures.clone(),
            ))
            .collect()
    }

    unsafe fn free(&mut self, _cbufs: Vec<CommandBuffer>) {
        // Just let the command buffers drop
    }
}
