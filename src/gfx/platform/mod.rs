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

#[cfg(glfw)] pub use Glfw = self::glfw::GlfwGraphicsContext;
// #[cfg(sdl2)] pub use Sdl2 = sdl2::Sdl2Platform; // TODO
// #[cfg(d3d)] pub use D3d = d3d::D3dPlatform; // TODO
//
#[cfg(glfw)] mod glfw;
// #[cfg(sdl2)] mod sdl2; // TODO
// #[cfg(d3d)] mod d3d; // TODO

pub enum GlApi {}
pub enum D3dApi {}

pub trait GraphicsContext<Api> {
    fn swap_buffers(&self);
    fn make_current(&self);
}

pub trait GlProvider {
	fn get_proc_address(&self, &str) -> *const ::libc::c_void;
	fn is_extension_supported(&self, &str) -> bool;
}
