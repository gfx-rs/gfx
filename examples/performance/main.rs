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

extern crate cgmath;
#[macro_use]
extern crate gfx;
extern crate gfx_core;
extern crate gfx_device_gl;
extern crate gfx_gl as gl;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::{Adapter, CommandQueue, GraphicsPoolExt, Surface, SwapChain, SwapChainExt, WindowExt};
use gfx::format::{DepthStencil, Formatted, Rgba8 as ColorFormat};

use cgmath::{Deg, Matrix, Matrix3, Matrix4, Point3, Vector3, Vector4, SquareMatrix};
use gl::Gl;
use gl::types::*;
use std::{mem, ptr, str, env};
use std::str::FromStr;
use std::iter::repeat;
use std::ffi::CString;
use std::time::{Duration, Instant};
use gfx_device_gl::{Resources as R, Backend as B};
use gfx_core::Device;

gfx_defines!{
    vertex Vertex {
        pos: [f32; 3] = "a_Pos",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        transform: gfx::Global<[[f32; 4]; 4]> = "u_Transform",
        out_color: gfx::RenderTarget<ColorFormat> = "o_Color",
    }
}

static VERTEX_SRC: &'static [u8] = b"
    #version 150 core
    in vec3 a_Pos;
    uniform mat4 u_Transform;

    void main() {
        gl_Position = u_Transform * vec4(a_Pos, 1.0); 
   }
";

static FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core
    out vec4 o_Color;

    void main() {
        o_Color = vec4(1.0, 0.0, 0.0, 1.0);
    }
";

static VERTEX_DATA: &'static [Vertex] = &[
    Vertex { pos: [-1.0, 0.0, -1.0] },
    Vertex { pos: [ 1.0, 0.0, -1.0] },
    Vertex { pos: [-1.0, 0.0,  1.0] },
];

const CLEAR_COLOR: (f32, f32, f32, f32) = (0.3, 0.3, 0.3, 1.0);

//----------------------------------------

fn transform(x: i16, y: i16, proj_view: &Matrix4<f32>) -> Matrix4<f32> {
    let mut model = Matrix4::from(Matrix3::identity() * 0.05);
    model.w = Vector4::new(x as f32 * 0.10,
                           0f32,
                           y as f32 * 0.10,
                           1f32);
    proj_view * model
}

trait Renderer: Drop {
    fn render(&mut self, proj_view: &Matrix4<f32>);
    fn window(&mut self) -> &glutin::Window;
}

struct GFX {
    dimension: i16,
    window: gfx_window_glutin::Window,
    surface: gfx_window_glutin::Surface,
    swap_chain: gfx_window_glutin::SwapChain,
    queue: gfx::queue::GraphicsQueue<B>,
    pool: gfx::GraphicsCommandPool<B>,
    views: Vec<gfx::handle::RenderTargetView<R, ColorFormat>>,
    data: pipe::Data<R>,
    pso: gfx::PipelineState<R, pipe::Meta>,
    slice: gfx::Slice<R>,
}

impl GFX {
    fn new(builder: glutin::WindowBuilder, events_loop: &glutin::EventsLoop, dimension: i16) -> Self {
        use gfx::traits::FactoryExt;

        // Create window
        let events_loop = glutin::EventsLoop::new();
        let builder = glutin::WindowBuilder::new()
            .with_title("Triangle example".to_string())
            .with_dimensions(1024, 768)
            .with_vsync();
        let win = gfx_window_glutin::build(builder, &events_loop, ColorFormat::get_format(), DepthStencil::get_format());
        let mut window = gfx_window_glutin::Window::new(win);
        // Acquire surface and adapters
        let (mut surface, adapters) = window.get_surface_and_adapters();
        let queue_descs = adapters[0].get_queue_families().iter()
                                    .filter(|family| surface.supports_queue(&family) )
                                    .map(|family| { (family, 1) })
                                    .collect::<Vec<_>>();

        // Open device (factory and queues)
        let gfx::Device { mut factory, mut general_queues, mut graphics_queues, .. } = adapters[0].open(&queue_descs);
        let mut queue = if let Some(queue) = general_queues.pop() {
            queue.into()
        } else if let Some(queue) = graphics_queues.pop() {
            queue
        } else {
            panic!("Unable to find a matching general or graphics queue.");
        };

        // Create swapchain
        let config = gfx::SwapchainConfig::new()
                        .with_color::<ColorFormat>();
        let mut swap_chain = surface.build_swapchain(config, &queue);
        let views = swap_chain.create_color_views(&mut factory);

        let pso = factory.create_pipeline_simple(
            VERTEX_SRC, FRAGMENT_SRC,
            pipe::new()
        ).unwrap();

        let (vbuf, slice) = factory.create_vertex_buffer_with_slice(VERTEX_DATA,());
        let data = pipe::Data {
            vbuf: vbuf,
            transform: cgmath::Matrix4::identity().into(),
            out_color: views[0].clone(),
        };
        let pool = queue.create_graphics_pool(1);

        GFX {
            window,
            surface,
            swap_chain,
            queue,
            pool,
            views,
            dimension,
            data,
            pso,
            slice,
        }
    }
}

impl Renderer for GFX {
    fn render(&mut self, proj_view: &Matrix4<f32>) {
        // TODO: currently relaying on GL backend internals (sync)
        let start = Instant::now();

        self.pool.reset();
        let mut encoder = self.pool.acquire_graphics_encoder();
        encoder.clear(&self.data.out_color, [CLEAR_COLOR.0,
                                                  CLEAR_COLOR.1,
                                                  CLEAR_COLOR.2,
                                                  CLEAR_COLOR.3]);

        for x in (-self.dimension) ..self.dimension {
            for y in (-self.dimension) ..self.dimension {
                self.data.transform = transform(x, y, proj_view).into();
                encoder.draw(&self.slice, &self.pso, &self.data);
            }
        }

        let pre_submit = start.elapsed();
        encoder.flush(&mut self.queue.as_mut());
        let post_submit = start.elapsed();
        self.swap_chain.present(&mut self.queue, &[]);
        self.queue.cleanup();
        let swap = start.elapsed();

        println!("total time:\t\t{0:4.2}ms", duration_to_ms(swap));
        println!("\tcreate list:\t{0:4.2}ms", duration_to_ms(pre_submit));
        println!("\tsubmit:\t\t{0:4.2}ms", duration_to_ms(post_submit - pre_submit));
        println!("\tgpu wait:\t{0:4.2}ms", duration_to_ms(swap - post_submit));
    }
    fn window(&mut self) -> &glutin::Window { self.window.raw() }
}

impl Drop for GFX {
    fn drop(&mut self) {
    }
}

struct GL {
    dimension: i16,
    window: glutin::Window,
    gl:Gl,
    trans_uniform:GLint,
    vs:GLuint,
    fs:GLuint,
    program:GLuint,
    vbo:GLuint,
    vao:GLuint,
}

impl GL {
    fn new(builder: glutin::WindowBuilder, events_loop: &glutin::EventsLoop, dimension: i16) -> Self {
        fn compile_shader (gl:&Gl, src: &[u8], ty: GLenum) -> GLuint {
            unsafe {
                let shader = gl.CreateShader(ty);
                // Attempt to compile the shader
                gl.ShaderSource(shader, 1,
                                &(src.as_ptr() as *const i8),
                                &(src.len() as GLint));
                gl.CompileShader(shader);

                // Get the compile status
                let mut status = gl::FALSE as GLint;
                gl.GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

                // Fail on error
                if status != (gl::TRUE as GLint) {
                    let mut len: GLint = 0;
                    gl.GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);

                    // allocate a buffer of size (len - 1) to skip the trailing null character
                    let mut buf: Vec<u8> = repeat(0u8).take((len as usize).saturating_sub(1)).collect();
                    gl.GetShaderInfoLog(shader, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
                    panic!("{}", str::from_utf8(&buf).ok().expect("ShaderInfoLog not valid utf8"));
                }
                shader
            }
        };

        let window = builder.build(events_loop).unwrap();
        unsafe { window.make_current().unwrap() };
        let gl = Gl::load_with(|s| window.get_proc_address(s) as *const _);
        
        // Create GLSL shaders
        let vs = compile_shader(&gl, VERTEX_SRC, gl::VERTEX_SHADER);
        let fs = compile_shader(&gl, FRAGMENT_SRC, gl::FRAGMENT_SHADER);

        // Link program
        let program;
        unsafe {
            program = gl.CreateProgram();
            gl.AttachShader(program, vs);
            gl.AttachShader(program, fs);
            gl.LinkProgram(program);
            // Get the link status
            let mut status = gl::FALSE as GLint;
            gl.GetProgramiv(program, gl::LINK_STATUS, &mut status);

            // Fail on error
            if status != (gl::TRUE as GLint) {
                let mut len: GLint = 0;
                gl.GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);

                // allocate a buffer of size (len - 1) to skip the trailing null character
                let mut buf: Vec<u8> = repeat(0u8).take((len as usize).saturating_sub(1)).collect();
                gl.GetProgramInfoLog(program, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
                panic!("{}", str::from_utf8(&buf).ok().expect("ProgramInfoLog not valid utf8"));
            }
        }

        let mut vao = 0;
        let mut vbo = 0;

        let trans_uniform;
        unsafe {
            // Create Vertex Array Object
            gl.GenVertexArrays(1, &mut vao);
            gl.BindVertexArray(vao);

            // Create a Vertex Buffer Object and copy the vertex data to it
            gl.GenBuffers(1, &mut vbo);
            gl.BindBuffer(gl::ARRAY_BUFFER, vbo);

            gl.BufferData(gl::ARRAY_BUFFER,
                          (VERTEX_DATA.len() * mem::size_of::<Vertex>()) as GLsizeiptr,
                          mem::transmute(&VERTEX_DATA[0]),
                          gl::STATIC_DRAW);

            // Use shader program
            gl.UseProgram(program);
            let o_color = CString::new("o_Color").unwrap();
            gl.BindFragDataLocation(program, 0, o_color.as_bytes_with_nul().as_ptr() as *const i8);

            // Specify the layout of the vertex data
            let a_pos = CString::new("a_Pos").unwrap();
            gl.BindFragDataLocation(program, 0, a_pos.as_bytes_with_nul().as_ptr() as *const i8);

            let pos_attr = gl.GetAttribLocation(program, a_pos.as_ptr());
            gl.EnableVertexAttribArray(pos_attr as GLuint);
            gl.VertexAttribPointer(pos_attr as GLuint, 3, gl::FLOAT,
                                   gl::FALSE as GLboolean, 0, ptr::null());

            let u_transform = CString::new("u_Transform").unwrap();
            trans_uniform = gl.GetUniformLocation(program, u_transform.as_bytes_with_nul().as_ptr() as *const i8)
        };

        GL {
            window: window,            
            dimension: dimension,
            gl: gl,
            vs: vs,
            fs: fs,
            program: program,
            vbo: vbo,
            vao: vao,
            trans_uniform: trans_uniform,
        }
    }
}

fn duration_to_ms(dur: Duration) -> f64 {
    (dur.as_secs() * 1000) as f64 + dur.subsec_nanos() as f64 / 1000_000.0
}

impl Renderer for GL {
    fn render(&mut self, proj_view: &Matrix4<f32>) {
        let start = Instant::now();

        // Clear the screen to black
        unsafe {
            self.gl.ClearColor(CLEAR_COLOR.0, CLEAR_COLOR.1, CLEAR_COLOR.2, CLEAR_COLOR.3);
            self.gl.Clear(gl::COLOR_BUFFER_BIT);
        }
        
        for x in (-self.dimension) ..self.dimension {
            for y in (-self.dimension) ..self.dimension {
                let mat:Matrix4<f32> = transform(x, y, proj_view).into();

                unsafe {
                    self.gl.UniformMatrix4fv(self.trans_uniform,
                                             1,
                                             gl::FALSE,
                                             mat.as_ptr());
                    self.gl.DrawArrays(gl::TRIANGLES, 0, 3);
                }

            }
        }

        let submit = start.elapsed();

        // Swap buffers
        self.window.swap_buffers().unwrap();
        let swap = start.elapsed();

        println!("total time:\t\t{0:4.2}ms", duration_to_ms(swap));
        println!("\tsubmit:\t\t{0:4.2}ms", duration_to_ms(submit));
        println!("\tgpu wait:\t{0:4.2}ms", duration_to_ms(swap - submit));
    }
    fn window(&mut self) -> &glutin::Window { &self.window }
}

impl Drop for GL {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteProgram(self.program);
            self.gl.DeleteShader(self.fs);
            self.gl.DeleteShader(self.vs);
            self.gl.DeleteBuffers(1, &self.vbo);
            self.gl.DeleteVertexArrays(1, &self.vao);
        }
    }
}

fn main() {
    let ref mut args = env::args();
    let args_count = env::args().count();
    if args_count == 1 {
        println!("gfx-perf [gl|gfx] <size>");
        return;
    }

    let mode = args.nth(1).unwrap();
    let count: i32 = if args_count == 3 {
        FromStr::from_str(&args.next().unwrap()).ok()
    } else {
        None
    }.unwrap_or(10000);

    let count = ((count as f64).sqrt() / 2.) as i16;

    let events_loop = glutin::EventsLoop::new();
    let builder = glutin::WindowBuilder::new()
        .with_title("Performance example".to_string())
        .with_dimensions(800, 600)
        .with_vsync();

    let mut r: Box<Renderer>;
    match mode.as_ref() {
        "gfx" => r = Box::new(GFX::new(builder, &events_loop, count)),
        "gl" => r = Box::new(GL::new(builder, &events_loop, count)),
        x => {
            panic!("{} is not a known mode", x)
        }
    }

    let proj_view = {
        let view = Matrix4::look_at(
            Point3::new(0f32, 5.0, -5.0),
            Point3::new(0f32, 0.0, 0.0),
            Vector3::unit_z(),
        );

        let proj = {
            let aspect = {
                let (w, h) = r.window().get_inner_size().unwrap();
                w as f32 / h as f32
            };
            cgmath::perspective(Deg(45.0f32), aspect, 1.0, 10.0)
        };
        proj * view
    };

    println!("count is {}", count*count*4);

    let mut running = true;
    loop {
        events_loop.poll_events(|glutin::Event::WindowEvent{window_id: _, event}| {
            match event {
                glutin::WindowEvent::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape), _) |
                glutin::WindowEvent::Closed => running = false,
                _ => {},
            }
        });
        if !running {
            break;
        }
        r.render(&proj_view);
    }
}
