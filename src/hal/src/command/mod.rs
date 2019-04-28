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

use crate::queue::capability::{Capability, Supports};
use crate::Backend;

use std::borrow::Borrow;
use std::marker::PhantomData;

mod compute;
mod graphics;
mod raw;
mod render_pass;
mod transfer;

pub use self::graphics::*;
pub use self::raw::{
    ClearColorRaw, ClearDepthStencilRaw, ClearValueRaw, CommandBufferFlags,
    CommandBufferInheritanceInfo, DescriptorSetOffset, IntoRawCommandBuffer, Level as RawLevel,
    RawCommandBuffer,
};
pub use self::render_pass::*;
pub use self::transfer::*;

/// Trait indicating how many times a Submit object can be submitted to a command buffer.
pub trait Shot {}
/// Indicates a Submit that can only be submitted once.
pub enum OneShot {}
impl Shot for OneShot {}

/// Indicates a Submit that can be submitted multiple times.
pub enum MultiShot {}
impl Shot for MultiShot {}

/// A trait indicating the level of a command buffer.
pub trait Level {}

/// Indicates a primary command buffer.

/// Vulkan describes a primary command buffer as one which can be directly submitted
/// to a queue, and can execute `Secondary` command buffers.
pub enum Primary {}
impl Level for Primary {}

/// Indicates a secondary command buffer.
///
/// Vulkan describes a secondary command buffer as one which cannot be directly submitted
/// to a queue, but can be executed by a primary command buffer. This allows
/// multiple secondary command buffers to be constructed which do specific
/// things and can then be composed together into primary command buffers.
pub enum Secondary {}
impl Level for Secondary {}

/// A property of a command buffer to be submitted to a queue with specific capability.
pub trait Submittable<B: Backend, C: Capability, L: Level>: Borrow<B::CommandBuffer> {}

/// A convenience alias for not typing out the full signature of a secondary command buffer.
pub type SecondaryCommandBuffer<B, C, S = OneShot> = CommandBuffer<B, C, S, Secondary>;

/// A strongly-typed command buffer that will only implement methods that are valid for the operations
/// it supports.
#[derive(Debug)]
pub struct CommandBuffer<B: Backend, C, S = OneShot, L = Primary, R = <B as Backend>::CommandBuffer>
{
    pub(crate) raw: R,
    pub(crate) _marker: PhantomData<(B, C, S, L)>,
}

impl<B, C, S, L, R> Borrow<R> for CommandBuffer<B, C, S, L, R>
where
    R: RawCommandBuffer<B>,
    B: Backend<CommandBuffer = R>,
{
    fn borrow(&self) -> &B::CommandBuffer {
        &self.raw
    }
}

impl<B: Backend, C, K: Capability + Supports<C>, S, L: Level> Submittable<B, K, L>
    for CommandBuffer<B, C, S, L>
{
}

impl<B: Backend, C, S, L> IntoRawCommandBuffer<B, C> for CommandBuffer<B, C, S, L> {
    fn into_raw(self) -> B::CommandBuffer {
        self.raw
    }
}

impl<B: Backend, C> CommandBuffer<B, C, OneShot, Primary> {
    /// Begin recording a one-shot primary command buffer.
    pub unsafe fn begin(&mut self) {
        let flags = CommandBufferFlags::ONE_TIME_SUBMIT;
        self.raw
            .begin(flags, CommandBufferInheritanceInfo::default());
    }
}

impl<B: Backend, C> CommandBuffer<B, C, MultiShot, Primary> {
    /// Begin recording a multi-shot primary command buffer.
    pub unsafe fn begin(&mut self, allow_pending_resubmit: bool) {
        let flags = if allow_pending_resubmit {
            CommandBufferFlags::SIMULTANEOUS_USE
        } else {
            CommandBufferFlags::empty()
        };
        self.raw
            .begin(flags, CommandBufferInheritanceInfo::default());
    }
}

impl<B: Backend, C> CommandBuffer<B, C, OneShot, Secondary> {
    /// Begin recording a one-shot secondary command buffer.
    pub unsafe fn begin(&mut self, inheritance: CommandBufferInheritanceInfo<B>) {
        let flags = CommandBufferFlags::ONE_TIME_SUBMIT;
        self.raw.begin(flags, inheritance);
    }
}

impl<B: Backend, C> CommandBuffer<B, C, MultiShot, Secondary> {
    /// Begin recording a multi-shot secondary command buffer.
    pub unsafe fn begin(
        &mut self,
        allow_pending_resubmit: bool,
        inheritance: CommandBufferInheritanceInfo<B>,
    ) {
        let flags = if allow_pending_resubmit {
            CommandBufferFlags::SIMULTANEOUS_USE
        } else {
            CommandBufferFlags::empty()
        };
        self.raw.begin(flags, inheritance);
    }
}

impl<B: Backend, C, S: Shot, L: Level> CommandBuffer<B, C, S, L> {
    /// Create a new typed command buffer from a raw command pool.
    pub unsafe fn new(raw: B::CommandBuffer) -> Self {
        CommandBuffer {
            raw,
            _marker: PhantomData,
        }
    }

    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer must be reset to able to re-record commands.
    pub unsafe fn finish(&mut self) {
        self.raw.finish();
    }

    /// Empties the command buffer, optionally releasing all resources from the
    /// commands that have been submitted. The command buffer is moved back to
    /// the "initial" state.
    ///
    /// The command buffer must not be in the "pending" state. Additionally, the
    /// command pool must have been created with the RESET_INDIVIDUAL flag to be
    /// able to reset individual buffers.
    pub unsafe fn reset(&mut self, release_resources: bool) {
        self.raw.reset(release_resources);
    }

    /*
    /// Get a reference to the raw command buffer
    pub fn as_raw(&self) -> &B::CommandBuffer {
        &self.raw
    }

    /// Get a mutable reference to the raw command buffer
    pub fn as_raw_mut(&mut self) -> &mut B::CommandBuffer {
        &mut self.raw
    }*/

    /// Downgrade a command buffer to a lesser capability type.
    pub unsafe fn downgrade<D>(&mut self) -> &mut CommandBuffer<B, D, S>
    where
        C: Supports<D>,
    {
        ::std::mem::transmute(self)
    }
}

impl<B: Backend, C, S: Shot> CommandBuffer<B, C, S, Primary> {
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn execute_commands<'a, I, T, K>(&mut self, cmd_buffers: I)
    where
        K: Capability,
        T: 'a + Submittable<B, K, Secondary>,
        I: IntoIterator<Item = &'a T>,
        C: Supports<K>,
    {
        self.raw
            .execute_commands(cmd_buffers.into_iter().map(|cmb| cmb.borrow()));
    }
}
