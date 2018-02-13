//! DOC TODO

use Backend;
use queue::capability::Supports;
use std::marker::PhantomData;

mod compute;
mod graphics;
mod raw;
mod renderpass;
mod transfer;

pub use self::graphics::*;
pub use self::raw::{ClearValueRaw, ClearColorRaw, ClearDepthStencilRaw, RawCommandBuffer, CommandBufferFlags, Level as RawLevel};
pub use self::renderpass::*;
pub use self::transfer::*;

use std::borrow::Borrow;

/// Trait indicating how many times a Submit can be submitted.
pub trait Shot {
    /// DOC TODO
    const FLAGS: CommandBufferFlags;
}
/// Indicates a Submit that can only be submitted once.
pub enum OneShot { }
impl Shot for OneShot { const FLAGS: CommandBufferFlags = CommandBufferFlags::ONE_TIME_SUBMIT; }

/// Indicates a Submit that can be submitted multiple times.
pub enum MultiShot { }
impl Shot for MultiShot { const FLAGS: CommandBufferFlags = CommandBufferFlags::EMPTY; }

/// A trait indicating the level of a command buffer.
pub trait Level { }

/// Indicates a primary command buffer.
///
/// Vulkan describes a primary command buffer as one which can be directly submitted
/// to a queue, and can execute `Secondary` command buffers.
pub enum Primary { }
impl Level for Primary { }

/// Indicates a secondary command buffer.
///
/// Vulkan describes a secondary command buffer as one which cannot be directly submitted
/// to a queue, but can be executed by a primary command buffer.  This allows
/// multiple secondary command buffers to be constructed which do specific 
/// things, which can then be composed together into primary command buffers.
pub enum Secondary { }
impl Level for Secondary { }

/// Thread-safe finished command buffer for submission.
pub struct Submit<B: Backend, C, S, L>(pub(crate) B::CommandBuffer, pub(crate) PhantomData<(C, S, L)>);
impl<B: Backend, C, S, L> Submit<B, C, S, L> {
    fn new(buffer: B::CommandBuffer) -> Self {
        Submit(buffer, PhantomData)
    }
}
unsafe impl<B: Backend, C, S, L> Send for Submit<B, C, S, L> {}

/// A trait representing a command buffer that can be added to a `Submission`.
pub unsafe trait Submittable<'a, B: Backend, C, L: Level> {
    /// DOC TODO
    type RawBuffer: Borrow<B::CommandBuffer> + 'a;
    /// DOC TODO
    unsafe fn as_buffer(self) -> Self::RawBuffer;
}

unsafe impl<'a, B: Backend, C, L: Level> Submittable<'a, B, C, L> for Submit<B, C, OneShot, L> {
    type RawBuffer = B::CommandBuffer;
    unsafe fn as_buffer(self) -> B::CommandBuffer { self.0 }
}
unsafe impl<'a, B: Backend, C, L: Level> Submittable<'a, B, C, L> for &'a Submit<B, C, MultiShot, L> {
    type RawBuffer = &'a B::CommandBuffer;
    unsafe fn as_buffer(self) -> &'a B::CommandBuffer { &self.0 }
}

/// A convenience alias for not typing out the full signature of a secondary command buffer.
pub type SecondaryCommandBuffer<'a, B: Backend, C, S: Shot = OneShot> = CommandBuffer<'a, B, C, S, Secondary>;

/// Command buffer with compute, graphics and transfer functionality.
pub struct CommandBuffer<'a, B: Backend, C, S: Shot = OneShot, L: Level = Primary> {
    pub(crate) raw: &'a mut B::CommandBuffer,
    pub(crate) _marker: PhantomData<(C, S, L)>
}

impl<'a, B: Backend, C, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    /// Create a new typed command buffer from a raw command pool.
    pub unsafe fn new(raw: &'a mut B::CommandBuffer) -> Self {
        CommandBuffer {
            raw: raw,
            _marker: PhantomData,
        }
    }

    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(self) -> Submit<B, C, S, L> {
        Submit::new(self.raw.clone())
    }

    /// Downgrade a command buffer to a lesser capability type.
    ///
    /// This is safe as you can't `submit` downgraded version since `submit`
    /// requires `self` by move.
    pub fn downgrade<D>(&mut self) -> &mut CommandBuffer<'a, B, D, S>
    where
        C: Supports<D>
    {
        unsafe { ::std::mem::transmute(self) }
    }
}

impl<'a, B: Backend, C, S: Shot> CommandBuffer<'a, B, C, S, Primary> {
    /// DOC TODO
    pub fn execute_commands<I, K>(&mut self, submits: I)
    where
        I: IntoIterator,
        I::Item: Submittable<'a, B, K, Secondary>,
        C: Supports<K>,
    {
        let submits = submits.into_iter().collect::<Vec<_>>();
        self.raw.execute_commands(submits.into_iter().map(|submit| unsafe { submit.as_buffer() }));
    }
}

impl<'a, B: Backend, C, S: Shot, L: Level> Drop for CommandBuffer<'a, B, C, S, L> {
    fn drop(&mut self) {
        self.raw.finish();
    }
}
