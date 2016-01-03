// Copyright 2015 The Gfx-rs Developers.
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

//! Various helper macros.

mod pso;
mod structure;

#[macro_export]
macro_rules! gfx_format {
    ($name:ident = $surface:ident . $channel:ident) => {
        impl $crate::format::Formatted for $name {
            type Surface = $crate::format::$surface;
            type Channel = $crate::format::$channel;
        }
    }
}
