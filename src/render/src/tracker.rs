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

//! Automatic resource mapping and handling

use core::handle;
use Resources;

/// TODO
pub struct Tracker<R: Resources> {
	frame_handles: handle::Manager<R>,
    max_resource_count: Option<usize>,
}

impl<R: Resources> Tracker<R> {
	/// TODO
	pub fn new() -> Self {
		Tracker {
			frame_handles: handle::Manager::new(),
            max_resource_count: Some(999999),
		}
	}
	/// TODO
	pub fn pin_submitted_resources(&mut self, man: &handle::Manager<R>) {
		self.frame_handles.extend(man);
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
	}
}