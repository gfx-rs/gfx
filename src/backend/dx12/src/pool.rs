use std::sync::Arc;

use winapi::shared::winerror::SUCCEEDED;

use command::CommandBuffer;
use hal::{command, pool};
use native::command_list::CmdListType;
use {native, Backend, Shared};

pub struct RawCommandPool {
    pub(crate) allocators: Vec<native::CommandAllocator>,
    pub(crate) device: native::Device,
    pub(crate) list_type: CmdListType,
    pub(crate) shared: Arc<Shared>,
}

impl RawCommandPool {
    fn create_command_list(&mut self) -> (native::GraphicsCommandList, native::CommandAllocator) {
        let (command_allocator, hr) = self.device.create_command_allocator(self.list_type);

        // TODO: error handling
        if !SUCCEEDED(hr) {
            error!("error on command allocator creation: {:x}", hr);
        }

        // allocate command lists
        let (command_list, hr) = self.device.create_graphics_command_list(
            self.list_type,
            command_allocator,
            native::PipelineState::null(),
            0,
        );

        if !SUCCEEDED(hr) {
            error!("error on command list creation: {:x}", hr);
        }

        // Close command list as they are initiated as recording.
        // But only one command list can be recording for each allocator
        let _hr = command_list.close();

        self.allocators.push(command_allocator);

        (command_list, command_allocator)
    }

    pub(crate) fn destroy(self) {
        for allocator in self.allocators {
            unsafe { allocator.destroy(); }
        }
    }
}

unsafe impl Send for RawCommandPool {}
unsafe impl Sync for RawCommandPool {}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        self.allocators.iter_mut().for_each(|allocator| {
            unsafe {
                allocator.Reset();
            }
        })
    }

    fn allocate(&mut self, num: usize, level: command::RawLevel) -> Vec<CommandBuffer> {
        // TODO: Implement secondary buffers
        assert_eq!(level, command::RawLevel::Primary);
        (0..num)
            .map(|_| {
                let (command_list, command_allocator) = self.create_command_list();
                CommandBuffer::new(command_list, command_allocator, self.shared.clone())
            })
            .collect()
    }

    unsafe fn free(&mut self, cbufs: Vec<CommandBuffer>) {
        for mut cbuf in cbufs {
            cbuf.destroy();
        }
    }
}
