use wio::com::ComPtr;
use dxguid;
use std::ptr;
use std::os::raw::c_void;
use std::ops::DerefMut;
use winapi;

use core::{self, pool};
use core::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use command::{CommandBuffer, SubpassCommandBuffer};
use core::command::{GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer};
use {Backend, CommandQueue};

struct CommandAllocator {
    inner: ComPtr<winapi::ID3D12CommandAllocator>,
    device: ComPtr<winapi::ID3D12Device>,
    list_type: winapi::D3D12_COMMAND_LIST_TYPE,
}

impl CommandAllocator {
    fn from_queue(queue: &CommandQueue) -> CommandAllocator {
        // create command allocator
        let mut command_allocator: *mut winapi::ID3D12CommandAllocator = ptr::null_mut();
        let hr = unsafe {
            // Note: ID3D12Device interface is free-threaded, therefore this call is safe
            queue.device.as_mut().CreateCommandAllocator(
                queue.list_type,
                &dxguid::IID_ID3D12CommandAllocator,
                &mut command_allocator as *mut *mut _ as *mut *mut c_void,
            )
        };
        // TODO: error handling
        if !winapi::SUCCEEDED(hr) {
            error!("error on command allocator creation: {:x}", hr);
        }

        CommandAllocator {
            inner: unsafe { ComPtr::new(command_allocator) },
            device: queue.device.clone(),
            list_type: queue.list_type,
        }
    }

    // Reset command allocator
    fn reset(&mut self) {
        unsafe { self.inner.Reset(); }
    }

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

unsafe impl Send for CommandAllocator { }

pub struct RawCommandPool {
    allocator: CommandAllocator,
    command_lists: Vec<CommandBuffer>,
    next_list: usize,
}

unsafe impl Send for RawCommandPool { }

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        // reset only allocator, as command lists will be reset on acquire.
        self.allocator.reset();
    }

    fn reserve(&mut self, additional: usize) {
        self.command_lists.reserve(additional);
        for _ in 0..additional {
            let command_list = self.allocator.create_command_list();
            self.command_lists.push(CommandBuffer { raw : command_list });
        }
    }

    unsafe fn from_queue<Q>(mut queue: Q, capacity: usize) -> RawCommandPool
    where Q: AsRef<CommandQueue>
    {
        let mut pool = RawCommandPool {
            allocator: CommandAllocator::from_queue(queue.as_ref()),
            command_lists: Vec::new(),
            next_list: 0,
        };

        pool.reserve(capacity);
        pool
    }

    unsafe fn acquire_command_buffer(&mut self) -> &mut CommandBuffer {
        let available_lists = self.command_lists.len() as isize - self.next_list as isize;
        if available_lists <= 0 {
            self.reserve((-available_lists) as usize + 1);
        }

        let mut list = &mut self.command_lists[self.next_list];
        self.next_list += 1;

        // reset to initial state
        unsafe { list.raw.as_mut().Reset(self.allocator.inner.as_mut() as *mut _, ptr::null_mut()); }
        list
    }
}


pub struct SubpassCommandPool {

}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {
    /*
    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, additional: usize) {
        unimplemented!()
    }

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, SubpassCommandBuffer> {
        unimplemented!()
    }

    fn from_queue<Q>(mut queue: Q, capacity: usize) -> SubpassCommandPool
        where Q: Compatible<GraphicsQueue<Backend>> + AsRef<CommandQueue>
    {
        unimplemented!()
    }
    */
}
