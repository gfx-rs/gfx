use std::sync::Arc;

use winapi::shared::winerror::SUCCEEDED;

use command::CommandBuffer;
use hal::{command, pool};
use {Backend, Shared};

use bal_dx12;
use bal_dx12::native::command_list::CmdListType;
use bal_dx12::native::{CommandAllocator, GraphicsCommandList};

pub struct RawCommandPool {
    pub(crate) raw: CommandAllocator,
    pub(crate) device: bal_dx12::native::Device,
    pub(crate) list_type: CmdListType,
    pub(crate) shared: Arc<Shared>,
}

impl RawCommandPool {
    fn create_command_list(&mut self) -> GraphicsCommandList {
        let (command_list, hr) = self.device.create_graphics_command_list(
            CmdListType::Direct,
            self.raw,
            bal_dx12::native::PipelineState::null(),
            0,
        );
        // TODO: error handling
        if !SUCCEEDED(hr) {
            error!("error on command list creation: {:x}", hr);
        }

        // Close command list as they are initiated as recording.
        // But only one command list can be recording for each allocator
        let _hr = command_list.close(); // TODO: error handling

        command_list
    }
}

unsafe impl Send for RawCommandPool {}
unsafe impl Sync for RawCommandPool {}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unsafe {
            self.raw.Reset();
        }
    }

    fn allocate(&mut self, num: usize, level: command::RawLevel) -> Vec<CommandBuffer> {
        // TODO: Implement secondary buffers
        assert_eq!(level, command::RawLevel::Primary);
        (0..num)
            .map(|_| CommandBuffer::new(self.create_command_list(), self.raw, self.shared.clone()))
            .collect()
    }

    unsafe fn free(&mut self, mut cbufs: Vec<CommandBuffer>) {
        for mut cbuf in cbufs {
            cbuf.destroy();
        }
    }
}
