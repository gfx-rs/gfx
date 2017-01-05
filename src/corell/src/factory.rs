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

use Resources;

/// A `Factory` is responsible for creating and managing resources for the backend it was created
/// with. 
///
/// # Construction and Handling
/// A `Factory` is typically created along with other objects using a helper function of the
/// appropriate gfx_window module (e.g. gfx_window_glutin::init()).
///
/// This factory structure can then be used to create and manage different resources, like buffers,
/// shader programs and textures. See the individual methods for more information.
///
/// Also see the `FactoryExt` trait inside the `gfx` module for additional methods.
#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// 
    fn allocate_memory(&mut self);

    ///
    fn create_shader(&mut self, code: &[u8]);
}
