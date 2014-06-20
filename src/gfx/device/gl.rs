extern mod gl;

pub type BufferRaw = gl::types::GLuint;

pub struct Device;

impl Device {
	pub fn new() -> Device {
		Device
	}
}