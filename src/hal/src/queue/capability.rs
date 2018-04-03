//! Type system encoded queue capabilities.
use queue::QueueType;

/// General capability, supporting graphics, compute and transfer operations.
pub enum General {}
/// Graphics capability, supporting graphics and transfer operations.
pub enum Graphics {}
/// Compute capability, supporting compute and transfer operations.
pub enum Compute {}
/// Transfer capability, supporting only transfer operations.
pub enum Transfer {}

/// Graphics or compute capability.
pub enum GraphicsOrCompute {}

/// A Capability is an object that specifies what kind of operations
/// a queue type performs, allowing what types support what queue operations
/// to be described at runtime by the type system.
pub trait Capability {
    /// Return true if this type level capability is supported by
    /// a run-time queue type.
    fn supported_by(qt: QueueType) -> bool;
}
impl Capability for General {
    fn supported_by(qt: QueueType) -> bool {
        match qt {
            QueueType::General => true,
            _ => false,
        }
    }
}
impl Capability for Graphics {
    fn supported_by(qt: QueueType) -> bool {
        match qt {
            QueueType::General |
            QueueType::Graphics => true,
            _ => false,
        }
    }
}
impl Capability for Compute {
    fn supported_by(qt: QueueType) -> bool {
        match qt {
            QueueType::General |
            QueueType::Compute => true,
            _ => false,
        }
    }
}
impl Capability for Transfer {
    fn supported_by(qt: QueueType) -> bool {
        match qt {
            _ => true
        }
    }
}

/// A trait that indicates that a particular type of queue supports
/// a particular `Capability`.
pub trait Supports<T> { }
impl<T> Supports<T> for T { }
impl Supports<Graphics> for General { }
impl Supports<Compute> for General { }
impl Supports<Transfer> for General { }
impl Supports<Transfer> for Graphics { }
impl Supports<Transfer> for Compute { }

impl Supports<GraphicsOrCompute> for General { }
impl Supports<GraphicsOrCompute> for Graphics { }
impl Supports<GraphicsOrCompute> for Compute { }

/// Encoding the minimal capability to support a combination of other capabilities.
pub trait Upper {
    /// Resulting minimal required capability.
    type Result;
}

impl<T> Upper for (T, T) { type Result = T; }
impl Upper for (General,  Graphics) { type Result = General; }
impl Upper for (General,  Compute)  { type Result = General; }
impl Upper for (General,  Transfer) { type Result = General; }
impl Upper for (Graphics, General)  { type Result = General; }
impl Upper for (Graphics, Compute)  { type Result = General; }
impl Upper for (Graphics, Transfer) { type Result = Graphics; }
impl Upper for (Compute,  General)  { type Result = General; }
impl Upper for (Compute,  Graphics) { type Result = General; }
impl Upper for (Compute,  Transfer) { type Result = Compute; }
impl Upper for (Transfer, General)  { type Result = General; }
impl Upper for (Transfer, Graphics) { type Result = Graphics; }
impl Upper for (Transfer, Compute)  { type Result = Compute; }
