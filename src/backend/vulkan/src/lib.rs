// Copyright 2016 The Gfx-rs Developers.
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

extern crate gfx_core;

mod vk {
    #![allow(dead_code)]
    #![allow(non_upper_case_globals)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    include!(concat!(env!("OUT_DIR"), "/vk_bindings.rs"));
}

pub struct Backend {
    instance: vk::Instance,
}