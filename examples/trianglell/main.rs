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

extern crate gfx_corell;

#[cfg(target_os = "windows")]
extern crate gfx_device_dx12ll as dx12;
#[cfg(feature = "vulkan")]
extern crate gfx_device_vulkanll as vulkan;

extern crate winit;

use gfx_corell::{Instance, PhysicalDevice};

fn main() {
    let window = winit::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title("triangle (Low Level)".to_string())
        .build()
        .unwrap();

    let instance = dx12::Instance::create();
    for device in instance.enumerate_physical_devices() {
        println!("{:?}", device.get_info());
    }

    'main: loop {
        for event in window.poll_events() {
            match event {
                winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
                winit::Event::Closed => break 'main,
                _ => {},
            }
        }

        // swap_chain.present();
    }
}