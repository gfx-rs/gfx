//! Type system encoded queue capabilities.

/// General capability, supporting graphics, compute and transfer operations.
pub struct General;
/// Graphics capability, supporting graphics and transfer operations.
pub struct Graphics;
/// Compute capability, supporting compute and transfer operations.
pub struct Compute;
/// Transfer capability, supporting only transfer operations.
pub struct Transfer;

///
pub trait Supports<T> { }
impl<T> Supports<T> for T { }
impl Supports<Graphics> for General { }
impl Supports<Compute> for General { }
impl Supports<Transfer> for General { }
impl Supports<Transfer> for Graphics { }
impl Supports<Transfer> for Compute { }

///
pub trait SupportedBy<T> { }
impl<U, T> SupportedBy<T> for U where T: Supports<U> { }

/// Encoding the minimal capability to support a combination of other capabilities.
pub trait Upper {
    /// Resulting mininmal required capability.
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
