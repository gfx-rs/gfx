use std::{fmt, sync::Arc};

use winapi::shared::winerror;

use crate::{command::CommandBuffer, Backend, Shared};
use hal::{command, pool};

#[derive(Debug)]
enum CommandPoolAllocator {
    Shared(native::CommandAllocator),
    Individual(Vec<native::CommandAllocator>),
}

pub struct CommandPool {
    shared: Arc<Shared>,
    device: native::Device,
    list_type: native::CmdListType,
    create_flags: pool::CommandPoolCreateFlags,
    allocator: CommandPoolAllocator,
    //lists: Vec<native::CommandList>,
}

impl fmt::Debug for CommandPool {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("CommandPool")
    }
}

impl CommandPool {
    pub(crate) fn new(
        device: native::Device,
        list_type: native::CmdListType,
        shared: &Arc<Shared>,
        create_flags: pool::CommandPoolCreateFlags,
    ) -> Self {
        let allocator = if create_flags.contains(pool::CommandPoolCreateFlags::RESET_INDIVIDUAL) {
            // Allocators are created per individual `ID3D12GraphicsCommandList`
            CommandPoolAllocator::Individual(Vec::new())
        } else {
            let (command_allocator, hr) = device.create_command_allocator(list_type);
            assert_eq!(
                hr,
                winerror::S_OK,
                "error on command allocator creation: {:x}",
                hr
            );
            CommandPoolAllocator::Shared(command_allocator)
        };
        CommandPool {
            shared: Arc::clone(shared),
            device,
            list_type,
            allocator,
            create_flags,
            //lists: Vec::new(),
        }
    }

    fn create_command_list(&mut self) -> (native::GraphicsCommandList, native::CommandAllocator) {
        let command_allocator = match self.allocator {
            CommandPoolAllocator::Shared(ref allocator) => allocator.clone(),
            CommandPoolAllocator::Individual(ref mut allocators) => {
                if let Some(command_allocator) = allocators.pop() {
                    command_allocator
                } else {
                    let (command_allocator, hr) =
                        self.device.create_command_allocator(self.list_type);
                    assert_eq!(
                        winerror::S_OK,
                        hr,
                        "error on command allocator creation: {:x}",
                        hr
                    );
                    command_allocator
                }
            }
        };

        // allocate command lists
        let (command_list, hr) = self.device.create_graphics_command_list(
            self.list_type,
            command_allocator,
            native::PipelineState::null(),
            0,
        );

        assert_eq!(
            hr,
            winerror::S_OK,
            "error on command list creation: {:x}",
            hr
        );

        // Close command list as they are initiated as recording.
        // But only one command list can be recording for each allocator
        let _hr = command_list.close();

        (command_list, command_allocator)
    }

    pub(crate) fn destroy(self) {
        match self.allocator {
            CommandPoolAllocator::Shared(ref allocator) => unsafe {
                allocator.destroy();
            },
            CommandPoolAllocator::Individual(ref allocators) => {
                for allocator in allocators.iter() {
                    unsafe {
                        allocator.destroy();
                    }
                }
            }
        }
    }
}

unsafe impl Send for CommandPool {}
unsafe impl Sync for CommandPool {}

impl pool::CommandPool<Backend> for CommandPool {
    unsafe fn reset(&mut self, _release_resources: bool) {
        match self.allocator {
            CommandPoolAllocator::Shared(ref allocator) => {
                allocator.Reset();
            }
            CommandPoolAllocator::Individual(ref mut allocators) => {
                for allocator in allocators.iter_mut() {
                    allocator.Reset();
                }
            }
        }
    }

    unsafe fn allocate_one(&mut self, level: command::Level) -> CommandBuffer {
        // TODO: Implement secondary buffers
        assert_eq!(level, command::Level::Primary);
        let (command_list, command_allocator) = self.create_command_list();
        CommandBuffer::new(
            command_list,
            command_allocator,
            self.shared.clone(),
            self.create_flags,
        )
    }

    unsafe fn free<I>(&mut self, cbufs: I)
    where
        I: IntoIterator<Item = CommandBuffer>,
    {
        for cbuf in cbufs {
            let allocator = cbuf.destroy();
            match self.allocator {
                CommandPoolAllocator::Shared(_) => {}
                CommandPoolAllocator::Individual(ref mut allocators) => {
                    allocator.Reset();
                    allocators.push(allocator);
                }
            }
        }
    }
}
