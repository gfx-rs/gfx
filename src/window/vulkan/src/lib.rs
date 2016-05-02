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

extern crate vk;
extern crate winit;
extern crate gfx_core;
extern crate gfx_device_vulkan;


pub fn init(builder: winit::WindowBuilder) -> (winit::Window) {
    use winit::os::unix::WindowExt;
    let _ = gfx_device_vulkan::create(&builder.window.title, 1, &[], &["VK_KHR_surface", "VK_KHR_xcb_surface"]);
    let win = builder.build().unwrap();
    //Surface::from_xlib(instance, win.get_xlib_display().unwrap(),
    //                   win.get_xlib_window().unwrap())
	win
}
