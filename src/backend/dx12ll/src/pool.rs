// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use comptr::ComPtr;
use dxguid;
use std::ptr;
use std::os::raw::c_void;
use std::ops::DerefMut;
use winapi;

use core::{self, pool};
use core::command::Encoder;
use core::{CommandPool, GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use native::{self, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use CommandQueue;

struct CommandAllocator {
    inner: ComPtr<winapi::ID3D12CommandAllocator>,
    
    device: ComPtr<winapi::ID3D12Device>,
    list_type: winapi::D3D12_COMMAND_LIST_TYPE,
}

impl CommandAllocator {
    fn from_queue(queue: &mut CommandQueue) -> CommandAllocator {
        // create command allocator
        let mut command_allocator = ComPtr::<winapi::ID3D12CommandAllocator>::new(ptr::null_mut());
        let hr = unsafe {
            queue.device.CreateCommandAllocator(
                queue.list_type,
                &dxguid::IID_ID3D12CommandAllocator,
                command_allocator.as_mut() as *mut *mut _ as *mut *mut c_void,
            )
        };
        // TODO: error handling
        if !winapi::SUCCEEDED(hr) {
            error!("error on command allocator creation: {:x}", hr);
        }

        CommandAllocator {
            inner: command_allocator,
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
        let mut command_list = ComPtr::<winapi::ID3D12GraphicsCommandList>::new(ptr::null_mut());
        let hr = unsafe {
            self.device.CreateCommandList(
                0, // single gpu only atm
                self.list_type,
                self.inner.as_mut_ptr(),
                ptr::null_mut(),
                &dxguid::IID_ID3D12GraphicsCommandList,
                command_list.as_mut() as *mut *mut _ as *mut *mut c_void,
            )
        };

        // TODO: error handling
        if !winapi::SUCCEEDED(hr) {
            error!("error on command list creation: {:x}", hr);
        }

        // Close command list as they are initiated as recording.
        // But only one command list can be recording for each allocator
        unsafe { command_list.Close(); }

        command_list
    }
}

macro_rules! impl_pool {
    ($pool:ident, $queue:ident, $buffer:ident) => (
        pub struct $pool {
            allocator: CommandAllocator,
            command_lists: Vec<$buffer>,
            next_list: usize,
        }

        impl core::CommandPool for $pool {
            type Queue = CommandQueue;
            type PoolBuffer = $buffer;

            fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, $buffer> {
                let available_lists = self.command_lists.len() as isize - self.next_list as isize;
                if available_lists <= 0 {
                    self.reserve((-available_lists) as usize + 1);
                }

                let list = &mut self.command_lists[self.next_list];
                self.next_list += 1;

                // reset to initial state
                unsafe { list.0.inner.Reset(self.allocator.inner.as_mut_ptr(), ptr::null_mut()); }
                unsafe { Encoder::new(list) }
            }

            fn reset(&mut self) {
                // reset only allocator, as command lists will be reset on acquire.
                self.allocator.reset();
            }

            fn reserve(&mut self, additional: usize) {
                self.command_lists.reserve(additional);
                for _ in 0..additional {
                    let command_list = self.allocator.create_command_list();
                    self.command_lists.push(
                        $buffer(
                            native::CommandBuffer { inner : command_list }
                        ));
                }
            }
        }

        impl pool::$pool for $pool {
            fn from_queue<Q>(queue: &mut Q, capacity: usize) -> $pool
                where Q: Into<$queue<CommandQueue>> + DerefMut<Target=CommandQueue>
            {
                let mut pool = $pool {
                    allocator: CommandAllocator::from_queue(queue),
                    command_lists: Vec::new(),
                    next_list: 0,
                };

                pool.reserve(capacity);
                pool
            }
        }
    )
}

impl_pool!{ GeneralCommandPool, GeneralQueue, GeneralCommandBuffer }
impl_pool!{ GraphicsCommandPool, GraphicsQueue, GraphicsCommandBuffer }
impl_pool!{ ComputeCommandPool, ComputeQueue, ComputeCommandBuffer }
impl_pool!{ TransferCommandPool, TransferQueue, TransferCommandBuffer }
impl_pool!{ SubpassCommandPool, GraphicsQueue, SubpassCommandBuffer }
