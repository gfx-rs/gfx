//! Command buffers.
//!
//! A command buffer collects a list of commands to be submitted to the device.
//! Each command buffer has specific capabilities for graphics, compute or transfer operations,
//! and can be either a "primary" command buffer or a "secondary" command buffer.  Operations
//! always start from a primary command buffer, but a primary command buffer can contain calls
//! to secondary command buffers that contain snippets of commands that do specific things, similar
//! to function calls.
//!
//! All the possible commands are implemented in the `RawCommandBuffer` trait, and then the `CommandBuffer`
//! and related types make a generic, strongly-typed wrapper around it that only expose the methods that
//! are valid for the capabilities it provides.

// TODO: Document pipelines and subpasses better.

use Backend;
use queue::capability::Supports;
use std::marker::PhantomData;

mod compute;
mod graphics;
mod raw;
mod render_pass;
mod transfer;

pub use self::graphics::*;
pub use self::raw::{
    ClearValueRaw, ClearColorRaw, ClearDepthStencilRaw, DescriptorSetOffset,
    RawCommandBuffer, CommandBufferFlags, Level as RawLevel, CommandBufferInheritanceInfo,
};
pub use self::render_pass::*;
pub use self::transfer::*;

use std::borrow::{Cow};

/// Trait indicating how many times a Submit object can be submitted to a command buffer.
pub trait Shot {
    ///
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

/// Vulkan describes a primary command buffer as one which can be directly submitted
/// to a queue, and can execute `Secondary` command buffers.
pub enum Primary { }
impl Level for Primary { }

/// Indicates a secondary command buffer.
///
/// Vulkan describes a secondary command buffer as one which cannot be directly submitted
/// to a queue, but can be executed by a primary command buffer. This allows
/// multiple secondary command buffers to be constructed which do specific
/// things and can then be composed together into primary command buffers.
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
    /// Unwraps the object into its underlying command buffer.
    unsafe fn into_buffer(self) -> Cow<'a, B::CommandBuffer>;
}

unsafe impl<'a, B: Backend, C, L: Level> Submittable<'a, B, C, L> for Submit<B, C, OneShot, L> {
    unsafe fn into_buffer(self) -> Cow<'a, B::CommandBuffer> { Cow::Owned(self.0) }
}

unsafe impl<'a, B: Backend, C, L: Level> Submittable<'a, B, C, L> for &'a Submit<B, C, MultiShot, L> {
    unsafe fn into_buffer(self) -> Cow<'a, B::CommandBuffer> { Cow::Borrowed(&self.0) }
}

/// A convenience alias for not typing out the full signature of a secondary command buffer.
#[allow(type_alias_bounds)]
pub type SecondaryCommandBuffer<'a, B: Backend, C, S: Shot = OneShot> = CommandBuffer<'a, B, C, S, Secondary>;

/// A strongly-typed command buffer that will only implement methods that are valid for the operations
/// it supports.
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

    /// Get a reference to the raw command buffer
    pub fn as_raw(&self) -> &B::CommandBuffer {
        &*self.raw
    }

    /// Get a mutable reference to the raw command buffer
    pub fn as_raw_mut(&mut self) -> &mut B::CommandBuffer {
        self.raw
    }

    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(self) -> Submit<B, C, S, L> {
        self.raw.finish();
        let raw = self.raw.clone();

        ::std::mem::forget(self);

        Submit::new(raw)
    }

    /// Downgrade a command buffer to a lesser capability type.
    ///
    /// This is safe as a downgraded version can't be `submit`'ed
    /// since `submit` requires `self` by move.
    pub fn downgrade<D>(&mut self) -> &mut CommandBuffer<'a, B, D, S>
    where
        C: Supports<D>
    {
        unsafe { ::std::mem::transmute(self) }
    }
}

impl<'a, B: Backend, C, S: Shot> CommandBuffer<'a, B, C, S, Primary> {
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn execute_commands<I, K>(&mut self, submits: I)
    where
        I: IntoIterator,
        I::Item: Submittable<'a, B, K, Secondary>,
        C: Supports<K>,
    {
        let submits = submits.into_iter().collect::<Vec<_>>();
        self.raw.execute_commands(submits.into_iter().map(|submit| unsafe { submit.into_buffer() }));
    }
}

impl<'a, B: Backend, C, S: Shot, L: Level> Drop for CommandBuffer<'a, B, C, S, L> {
    fn drop(&mut self) {
        self.raw.finish();
    }
}

