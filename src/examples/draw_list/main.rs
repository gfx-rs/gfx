#![feature(phase)]
#![crate_name = "draw-list"]

#[phase(plugin)]
extern crate gfx_macros;
extern crate getopts;
extern crate gfx;
extern crate glinit = "gl-init-rs";
extern crate gl_init_platform;
extern crate native;

#[vertex_format]
struct Vertex {
	pos: [f32, ..2],
	color: [f32, ..3],
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
	#version 120
	attribute vec2 pos;
	attribute vec3 color;
	varying vec4 v_Color;
	void main() {
		v_Color = vec4(color, 1.0);
		gl_Position = vec4(pos, 0.0, 1.0);
	}
"
GLSL_150: b"
	#version 150 core
	in vec2 pos;
	in vec3 color;
	out vec4 v_Color;
	void main() {
		v_Color = vec4(color, 1.0);
		gl_Position = vec4(pos, 0.0, 1.0);
	}
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
	#version 120
	varying vec4 v_Color;
	void main() {
		gl_FragColor = v_Color;
	}
"
GLSL_150: b"
	#version 150 core
	in vec4 v_Color;
	out vec4 o_Color;
	void main() {
		o_Color = v_Color;
	}
"
};

type Device<'a> = gfx::DeviceType<&'a gl_init_platform::Window>;

struct App {
	x:int,
}

impl App {
	fn new(renderer: &mut gfx::Renderer, device: &mut Device, width: u16, height: u16) -> App {
		let frame = gfx::Frame::new(width as u16, height as u16);
		let state = gfx::DrawState::new();

		let vertex_data = vec![
			Vertex { pos: [ -0.5, -0.5 ], color: [1.0, 0.0, 0.0] },
			Vertex { pos: [ 0.5, -0.5 ], color: [0.0, 1.0, 0.0]  },
			Vertex { pos: [ 0.0, 0.5 ], color: [0.0, 0.0, 1.0]  }
		];

		let mesh = renderer.create_mesh(vertex_data);
		let program = renderer.create_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone());

		let clear = gfx::ClearData {
			color: Some(gfx::Color([0.3, 0.3, 0.3, 1.0])),
			depth: None,
			stencil: None,
		};

		App {
			x: 0,
		}
	}

	fn render(&mut self, device: &mut Device) {
		/*renderer.clear(clear, frame);
		renderer.draw(&mesh, mesh.get_slice(), &frame, &program, &state)
			.unwrap();
		renderer.end_frame();
		for err in renderer.errors() {
			println!("Renderer error: {}", err);
		}*/
	}
}

// We need to run on the main thread for GLFW, so ensure we are using the `native` runtime. This is
// technically not needed, since this is the default, but it's not guaranteed.
#[start]
fn start(argc: int, argv: *const *const u8) -> int {
	native::start(argc, argv, main)
}

fn main() {
	let window = gl_init_platform::Window::new().unwrap();
	window.set_title("DrawList example #gfx-rs!");
	unsafe { window.make_current() };

	let (w, h) = window.get_inner_size().unwrap();

	let (mut renderer, mut device) = gfx::build()
		.with_context(&window)
		.with_provider(&window)
		.with_queue_size(1)
		.create()
		.unwrap();

	let mut app = App::new(&mut renderer, &mut device, w as u16, h as u16);

	'main: loop {
		// quit when Esc is pressed.
		for event in window.poll_events() {
			match event {
				glinit::Pressed(glinit::Escape) => break 'main,
				glinit::Closed => break 'main,
				_ => {},
			}
		}
		app.render(&mut device);
	}
}
