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

//! Type system encoded queue capabilities.

/// General capability, supporting graphics, compute and transfer operations.
pub struct General;
/// Graphics capability, supporting graphics and transfer operations.
pub struct Graphics;
/// Compute capability, supporting compute and transfer operations.
pub struct Compute;
/// Transfer capability, supporting only transfer operations.
pub struct Transfer;

/// Tag trait, annotating the capability to a certain struct (e.g Submission).
pub trait Capability {
    ///
    type Capability;
}

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
