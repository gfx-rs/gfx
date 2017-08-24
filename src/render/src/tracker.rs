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