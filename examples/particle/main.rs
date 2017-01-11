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

extern crate rand;
#[macro_use]
extern crate gfx;
extern crate gfx_app;

use std::time::Instant;

pub use gfx_app::{ColorFormat, DepthFormat};
use gfx::{Bundle, ShaderSet, Primitive, buffer, Bind, Slice};
use gfx::state::Rasterizer;

// Declare the vertex format suitable for drawing,
// as well as the constants used by the shaders
// and the pipeline state object format.
gfx_defines!{
    // Data for each particle
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
        vel: [f32; 2] = "a_Vel",
        color: [f32; 4] = "a_Color",
    }

    // Aspect ratio to keep particles round
    constant Locals {
        aspect: f32 = "u_Aspect",
    }

    // Particle render pipeline
    pipeline particles {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        out_color: gfx::BlendTarget<ColorFormat> = ("Target0", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
    }
}


impl Vertex {
    // Construct new particles far away so they can't be seen initially
    fn new() -> Vertex {
        Vertex {
            pos: [std::f32::INFINITY, std::f32::INFINITY],
            vel: Default::default(),
            color: Default::default(),
        }
    }
}

//----------------------------------------
struct App<R: gfx::Resources>{
    bundle: Bundle<R, particles::Data<R>>,
    particles: Vec<Vertex>,
    aspect: f32,
    time_start: Instant,
}

fn create_shader_set<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F, vs_code: &[u8], gs_code: &[u8], ps_code: &[u8]) -> ShaderSet<R> {
    let vs = factory.create_shader_vertex(vs_code).expect("Failed to compile vertex shader");
    let gs = factory.create_shader_geometry(gs_code).expect("Failed to compile geometry shader");
    let ps = factory.create_shader_pixel(ps_code).expect("Failed to compile pixel shader");
    ShaderSet::Geometry(vs, gs, ps)
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(factory: &mut F, backend: gfx_app::shade::Backend,
           window_targets: gfx_app::WindowTargets<R>) -> Self {
        use gfx::traits::FactoryExt;

        // Compute the aspect ratio so that our particles aren't stretched
        let (width, height, _, _) = window_targets.color.get_dimensions();
        let aspect = (height as f32)/(width as f32);

        // Load in our vertex, geometry and pixel shaders
        let vs = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/particle_150.glslv"),
            hlsl_40:  include_bytes!("data/vs_particle.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let gs = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/particle_150.glslg"),
            hlsl_40:  include_bytes!("data/gs_particle.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let ps = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/particle_150.glslf"),
            hlsl_40:  include_bytes!("data/ps_particle.fx"),
            .. gfx_app::shade::Source::empty()
        };

        let shader_set = create_shader_set(
            factory,
            vs.select(backend).unwrap(),
            gs.select(backend).unwrap(),
            ps.select(backend).unwrap(),
        );

        // Create 4096 particles, using one point vertex per particle
        let mut particles = vec![Vertex::new(); 4096];

        // Create a dynamic vertex buffer to hold the particle data
        let vbuf = factory.create_buffer(particles.len(),
                                         buffer::Role::Vertex,
                                         gfx::memory::Usage::Dynamic,
                                         Bind::empty())
            .expect("Failed to create vertex buffer");
        let slice = Slice::new_match_vertex_buffer(&vbuf);

        // Construct our pipeline state
        let pso = factory.create_pipeline_state(
            &shader_set,
            Primitive::PointList,
            Rasterizer::new_fill(),
            particles::new()
        ).unwrap();

        let data = particles::Data {
            vbuf: vbuf,
            locals: factory.create_constant_buffer(1),
            out_color: window_targets.color,
        };

        // Initialize the particles with random colours
        // (the alpha value doubles as the particle's "remaining life")
        for p in particles.iter_mut() {
            p.color = [rand::random(), rand::random(), rand::random(), rand::random()];
        }

        App {
            bundle: Bundle::new(slice, pso, data),
            particles: particles,
            aspect: aspect,
            time_start: Instant::now(),
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        // Compute the time since last frame
        let delta = self.time_start.elapsed();
        self.time_start = Instant::now();
        let delta = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1000_000_000.0;

        // Acceleration due to gravity
        let acc = -10.0;

        for p in self.particles.iter_mut() {
            // Particles are under constant acceleration, so use the exact formulae:
            // s = ut + 1/2 at^2
            // v = u + at
            p.pos[0] += p.vel[0]*delta;
            p.pos[1] += p.vel[1]*delta + 0.5*acc*delta*delta;
            p.vel[1] += acc*delta;
            // Fade out steadily
            p.color[3] -= 1.0*delta;

            // If particle has faded out completely
            if p.color[3] <= 0.0 {
                // Put it back at the emitter with new random parameters
                p.color[3] += 1.0;
                p.pos = [0.0, -1.0];
                let angle: f32 = (rand::random::<f32>()-0.5)*std::f32::consts::PI*0.2;
                let speed: f32 = rand::random::<f32>()*4.0 + 3.0;
                p.vel = [angle.sin()*speed, angle.cos()*speed];
            }
        }

        // Pass in the aspect ratio to the geometry shader
        let locals = Locals { aspect: self.aspect };
        encoder.update_constant_buffer(&self.bundle.data.locals, &locals);
        // Update the vertex data with the changes to the particles array
        encoder.update_buffer(&self.bundle.data.vbuf, &self.particles, 0).unwrap();
        // Clear the background to dark blue
        encoder.clear(&self.bundle.data.out_color, [0.1, 0.2, 0.3, 1.0]);
        // Draw the particles!
        self.bundle.encode(encoder);
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
        self.bundle.data.out_color = window_targets.color;
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_simple("Particle example");
}
