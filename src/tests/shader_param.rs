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

use secret_lib;

// Test all features
#[shader_param(MyBatch)]
struct MyParam {
    a: i32,
    b: [f32, ..4],
    c: secret_lib::gfx::shade::TextureParam,
    d: secret_lib::gfx::RawBufferHandle,
    #[unused]
    e: f32,
    #[name = "a_f"]
    f: [f32, ..4],
}

// Test that there are no conflicts between the two reexport modules
#[shader_param(MyBatch2)]
struct MyParam2 {
    a: i32,
    b: secret_lib::gfx::shade::TextureParam,
}

#[test]
fn test_shader_param() {
    //empty
}
