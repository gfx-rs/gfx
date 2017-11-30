//!

use Backend;
use queue::capability::Supports;
use std::marker::PhantomData;

mod compute;
mod graphics;
mod raw;
mod renderpass;
mod transfer;

pub use self::graphics::*;
pub use self::raw::RawCommandBuffer;
pub use self::renderpass::*;
pub use self::transfer::*;

/// Thread-safe finished command buffer for submission.
pub struct Submit<B: Backend, C>(pub(crate) B::CommandBuffer, PhantomData<C>);
unsafe impl<B: Backend, C> Send for Submit<B, C> {}

impl<B: Backend, C> Submit<B, C> {
    ///
    pub unsafe fn new(buffer: CommandBuffer<B, C>) -> Self {
        Submit(buffer.raw.clone(), PhantomData)
    }
}

/// Command buffer with compute, graphics and transfer functionality.
pub struct CommandBuffer<'a, B: Backend, C> {
    pub(crate) raw: &'a mut B::CommandBuffer,
    _capability: PhantomData<C>,
}

impl<'a, B: Backend, C> CommandBuffer<'a, B, C> {
    /// Create a new typed command buffer from a raw command pool.
    pub unsafe fn new(raw: &'a mut B::CommandBuffer) -> Self {
        CommandBuffer {
            raw,
            _capability: PhantomData,
        }
    }

    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(self) -> Submit<B, C> {
        unsafe { Submit::new(self) }
    }

    /// Downgrade a command buffer to a lesser capability type.
    /// 
    /// This is safe as you can't `submit` downgraded version since `submit`
    /// requires `self` by move.
    pub fn downgrade<D>(&mut self) -> &mut CommandBuffer<'a, B, D>
    where
        C: Supports<D>
    {
        unsafe { ::std::mem::transmute(self) }
    }
}

impl<'a, B: Backend, C> Drop for CommandBuffer<'a, B, C> {
    fn drop(&mut self) {
        self.raw.finish();
    }
}
