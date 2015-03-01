// Copyright 2014 The Gfx-rs Developers.
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

#![feature(plugin, custom_attribute)]
#![plugin(gfx_macros)]

mod secret_lib;

use secret_lib::gfx;

type R = ();
// Test all features
#[shader_param]
#[allow(dead_code)]
struct TestParam<R: gfx::Resources> {
    a: i32,
    b: [f32; 4],
    c: gfx::shade::TextureParam<R>,
    d: gfx::RawBufferHandle<R>,
    e: f32,
    #[name = "a_f"]
    f: [f32; 4],
}

#[test]
fn test_link_copy() {
    // testing if RefBatch is copyable
    fn _is_copy<T: Copy>(_t: T) {}
    fn _ref_copy(batch: gfx::batch::RefBatch<TestParam<R>>) {
        _is_copy(batch)
    }
}

#[test]
fn test_shader_param() {
    // testing if RefBatch can be constructed
    let _ref: gfx::batch::RefBatch<TestParam<R>>;
    // testing if OwnedBatch can be constructed
    let _owned: gfx::batch::OwnedBatch<TestParam<R>>;
}
