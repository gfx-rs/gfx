// Copyright 2015 The Gfx-rs Developers.
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
extern crate genmesh;
#[macro_use]
extern crate gfx;
extern crate gfx_app;
extern crate image;
extern crate noise;
extern crate rand;
extern crate winit;

pub use gfx_app::{ColorFormat, DepthFormat};

use cgmath::{Deg, Matrix4, Point3, SquareMatrix, Vector3};
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{Plane, SharedVertex, IndexedPolygon};
use gfx::traits::FactoryExt;
use std::io::Cursor;

// this is a value based on a max buffer size (and hence tilemap size) of 64x64
// I imagine you would have a max buffer length, with multiple TileMap instances
// of varying sizes based on current screen resolution
pub const TILEMAP_BUF_LENGTH: usize = 4096;

// texture loading boilerplate
pub fn load_texture<R, F>(factory: &mut F, data: &[u8])
                    -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String>
        where R: gfx::Resources, F: gfx::Factory<R>
{
    use gfx::format::Rgba8;
    use gfx::texture as t;
    let img = image::load(Cursor::new(data), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = t::Kind::D2(width as t::Size, height as t::Size, t::AaMode::Single);
    let (_, view) = factory.create_texture_immutable_u8::<Rgba8>(kind, &[&img]).unwrap();
    Ok(view)
}

// Actual tilemap data that makes up the elements of the UBO.
// NOTE: It may be a bug, but it appears that
// [f32;2] won't work as UBO data. Possibly an issue with
// binding generation
gfx_defines!{
    constant TileMapData {
        data: [f32; 4] = "data",
    }

    constant ProjectionStuff {
        model: [[f32; 4]; 4] = "u_Model",
        view: [[f32; 4]; 4] = "u_View",
        proj: [[f32; 4]; 4] = "u_Proj",
    }

    constant TilemapStuff {
        world_size: [f32; 4] = "u_WorldSize",
        tilesheet_size: [f32; 4] = "u_TilesheetSize",
        offsets: [f32; 2] = "u_TileOffsets",
    }

    vertex VertexData {
        pos: [f32; 3] = "a_Pos",
        buf_pos: [f32; 2] = "a_BufPos",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<VertexData> = (),
        projection_cb: gfx::ConstantBuffer<ProjectionStuff> = "b_VsLocals",
        // tilemap stuff
        tilemap: gfx::ConstantBuffer<TileMapData> = "b_TileMap",
        tilemap_cb: gfx::ConstantBuffer<TilemapStuff> = "b_PsLocals",
        tilesheet: gfx::TextureSampler<[f32; 4]> = "t_TileSheet",
        // output
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
        out_depth: gfx::DepthTarget<DepthFormat> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

impl TileMapData {
    pub fn new_empty() -> TileMapData {
        TileMapData { data: [0.0, 0.0, 0.0, 0.0] }
    }
    pub fn new(data: [f32; 4]) -> TileMapData {
        TileMapData { data: data }
    }
}

// Abstracts the plane mesh and uniform data
// Also holds a Vec<TileMapData> as a working data
// set for consumers
pub struct TileMapPlane<R> where R: gfx::Resources {
    pub params: pipe::Data<R>,
    pub slice: gfx::Slice<R>,
    proj_stuff: ProjectionStuff,
    proj_dirty: bool,
    tm_stuff: TilemapStuff,
    tm_dirty: bool,
    pub data: Vec<TileMapData>,
}

impl<R> TileMapPlane<R> where R: gfx::Resources {
    pub fn new<F>(factory: &mut F, width: usize, height: usize, tile_size: usize,
                  targets: gfx_app::WindowTargets<R>)
               -> TileMapPlane<R> where F: gfx::Factory<R> {
        // charmap info
        let half_width = (tile_size * width) / 2;
        let half_height = (tile_size * height) / 2;
        let total_size = width*height;

        // tilesheet info
        let tilesheet_bytes = &include_bytes!("scifitiles-sheet_0.png")[..];
        let tilesheet_width = 14;
        let tilesheet_height = 9;
        let tilesheet_tilesize = 32;

        let tilesheet_total_width = tilesheet_width * tilesheet_tilesize;
        let tilesheet_total_height = tilesheet_height * tilesheet_tilesize;
        // set up vertex data
        let plane = Plane::subdivide(width, width);

        // law out the vertices of the plane slice based on the configured tile size information,
        // setting the a_BufPos vertex data for the vertex shader (that ultimate gets passed through
        // to the frag shader as a varying, used to determine the "current tile" and the frag's offset,
        // which is used to calculate the displayed frag color)
        let vertex_data: Vec<VertexData> = plane.shared_vertex_iter()
            .map(|(raw_x, raw_y)| {
                let vertex_x = half_width as f32 * raw_x;
                let vertex_y = half_height as f32 * raw_y;

                let u_pos = (1.0 + raw_x) / 2.0;
                let v_pos = (1.0 + raw_y) / 2.0;
                let tilemap_x = (u_pos * width as f32).floor();
                let tilemap_y = (v_pos * height as f32).floor();

                VertexData {
                    pos: [vertex_x, vertex_y, 0.0],
                    buf_pos: [tilemap_x as f32, tilemap_y as f32]
                }
            })
            .collect();

        let index_data: Vec<u32> = plane.indexed_polygon_iter()
            .triangulate()
            .vertices()
            .map(|i| i as u32)
            .collect();

        let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data, &index_data[..]);

        let tile_texture = load_texture(factory, tilesheet_bytes).unwrap();

        let params = pipe::Data {
            vbuf: vbuf,
            projection_cb: factory.create_constant_buffer(1),
            tilemap: factory.create_constant_buffer(TILEMAP_BUF_LENGTH),
            tilemap_cb: factory.create_constant_buffer(1),
            tilesheet: (tile_texture, factory.create_sampler_linear()),
            out_color: targets.color,
            out_depth: targets.depth,
        };

        let mut charmap_data = Vec::with_capacity(total_size);
        for _ in 0..total_size {
            charmap_data.push(TileMapData::new_empty());
        }

        let view = Matrix4::look_at(
            Point3::new(0.0, 0.0, 800.0),
            Point3::new(0.0, 0.0, 0.0),
            Vector3::unit_y(),
        );

        TileMapPlane {
            slice: slice,
            params: params,
            proj_stuff: ProjectionStuff {
                model: Matrix4::identity().into(),
                view: view.into(),
                proj: cgmath::perspective(Deg(60.0f32), targets.aspect_ratio, 0.1, 4000.0).into(),
            },
            proj_dirty: true,
            tm_stuff: TilemapStuff {
                world_size: [width as f32, height as f32, tile_size as f32, 0.0],
                tilesheet_size: [tilesheet_width as f32, tilesheet_height as f32, tilesheet_total_width as f32, tilesheet_total_height as f32],
                offsets: [0.0, 0.0],
            },
            tm_dirty: true,
            data: charmap_data,
        }
    }

    fn resize(&mut self, targets: gfx_app::WindowTargets<R>) {
        self.params.out_color = targets.color;
        self.params.out_depth = targets.depth;
        self.proj_stuff.proj = cgmath::perspective(Deg(60.0f32), targets.aspect_ratio, 0.1, 4000.0).into();
        self.proj_dirty = true;
    }

    fn prepare_buffers<C>(&mut self, encoder: &mut gfx::Encoder<R, C>, update_data: bool) where C: gfx::CommandBuffer<R> {
        if update_data {
            encoder.update_buffer(&self.params.tilemap, &self.data, 0).unwrap();
        }
        if self.proj_dirty {
            encoder.update_constant_buffer(&self.params.projection_cb, &self.proj_stuff);
            self.proj_dirty = false;
        }
        if self.tm_dirty {
            encoder.update_constant_buffer(&self.params.tilemap_cb, &self.tm_stuff);
            self.tm_dirty = false;
        }
    }
    fn clear<C>(&self, encoder: &mut gfx::Encoder<R, C>) where C: gfx::CommandBuffer<R> {
        encoder.clear(&self.params.out_color,
            [16.0 / 256.0, 14.0 / 256.0, 22.0 / 256.0, 1.0]);
        encoder.clear_depth(&self.params.out_depth, 1.0);
    }
    pub fn update_view(&mut self, view: &Matrix4<f32>) {
        self.proj_stuff.view = view.clone().into();
        self.proj_dirty = true;
    }
    pub fn update_x_offset(&mut self, amt: f32) {
        self.tm_stuff.offsets[0] = amt;
        self.tm_dirty = true;
    }
    pub fn update_y_offset(&mut self, amt: f32) {
        self.tm_stuff.offsets[1] = amt;
        self.tm_dirty = true;
    }
}

#[derive(Clone)]
struct InputState {
    distance: f32,
    x_pos: f32,
    y_pos: f32,
    move_amt: f32,
    offset_amt: f32,
}

// Encapsulates the TileMapPlane and holds state for the current
// visible set of tiles. Is responsible for updating the UBO
// within the TileMapData when the visible set of tiles changes
pub struct TileMap<R> where R: gfx::Resources {
    pub tiles: Vec<TileMapData>,
    pso: gfx::PipelineState<R, pipe::Meta>,
    tilemap_plane: TileMapPlane<R>,
    tile_size: f32,
    tilemap_size: [usize; 2],
    charmap_size: [usize; 2],
    limit_coords: [usize; 2],
    focus_coords: [usize; 2],
    focus_dirty: bool,
    input: InputState,
}

impl<R: gfx::Resources> TileMap<R> {
    pub fn set_focus(&mut self, focus: [usize; 2]) {
        if focus[0] <= self.limit_coords[0] && focus[1] <= self.limit_coords[1] {
            self.focus_coords = focus;
            let mut charmap_ypos = 0;
            for ypos in self.focus_coords[1] .. self.focus_coords[1]+self.charmap_size[1] {
                let mut charmap_xpos = 0;
                for xpos in self.focus_coords[0] .. self.focus_coords[0]+self.charmap_size[0] {
                    let tile_idx = (ypos * self.tilemap_size[0]) + xpos;
                    let charmap_idx = (charmap_ypos * self.charmap_size[0]) + charmap_xpos;
                    self.tilemap_plane.data[charmap_idx] = self.tiles[tile_idx];
                    charmap_xpos += 1;
                }
                charmap_ypos += 1;
            }
            self.focus_dirty = true;
        } else {
            panic!("tried to set focus to {:?} with tilemap_size of {:?}", focus, self.tilemap_size);
        }
    }
    pub fn apply_x_offset(&mut self, offset_amt: f32) {
        let mut new_offset = self.tilemap_plane.tm_stuff.offsets[0] + offset_amt;
        let curr_focus = self.focus_coords;
        let new_x = if new_offset < 0.0 {
            // move down
            if self.focus_coords[0] == 0 {
                new_offset = 0.0;
                0
            } else {
                new_offset = self.tile_size + new_offset as f32;
                self.focus_coords[0] - 1
            }
        } else if self.focus_coords[0] == self.limit_coords[0] {
            // at top, no more offset
            new_offset = 0.0;
            self.focus_coords[0]
        } else if new_offset >= self.tile_size {
            new_offset = new_offset - self.tile_size as f32;
            self.focus_coords[0] + 1
        } else {
            // no move
            self.focus_coords[0]
        };
        if new_x != self.focus_coords[0] {
            self.set_focus([new_x, curr_focus[1]]);
        }
        self.tilemap_plane.update_x_offset(new_offset);
    }
    pub fn apply_y_offset(&mut self, offset_amt: f32) {
        let mut new_offset = self.tilemap_plane.tm_stuff.offsets[1] + offset_amt;
        let curr_focus = self.focus_coords;
        let new_y = if new_offset < 0.0 {
            // move down
            if self.focus_coords[1] == 0 {
                new_offset = 0.0;
                0
            } else {
                new_offset = self.tile_size + new_offset as f32;
                self.focus_coords[1] - 1
            }
        } else if self.focus_coords[1] == (self.tilemap_size[1] - self.charmap_size[1]) {
            // at top, no more offset
            new_offset = 0.0;
            self.focus_coords[1]
        } else if new_offset >= self.tile_size {
            new_offset = new_offset - self.tile_size as f32;
            self.focus_coords[1] + 1
        } else {
            // no move
            self.focus_coords[1]
        };
        if new_y != self.focus_coords[1] {
            self.set_focus([curr_focus[0], new_y]);
        }
        self.tilemap_plane.update_y_offset(new_offset);
    }
    fn calc_idx(&self, xpos: usize, ypos: usize) -> usize {
        (ypos * self.tilemap_size[0]) + xpos
    }
    pub fn set_tile(&mut self, xpos: usize, ypos: usize, data: [f32; 4]) {
        let idx = self.calc_idx(xpos, ypos);
        self.tiles[idx] = TileMapData::new(data);
    }
}


fn populate_tilemap<R>(tilemap: &mut TileMap<R>, tilemap_size: [usize; 2]) where R: gfx::Resources {
    // paper in with dummy data
    for ypos in 0 .. tilemap_size[1] {
        for xpos in 0 .. tilemap_size[0] {
            tilemap.set_tile(xpos, ypos, [1.0, 7.0, 0.0, 0.0]);
        }
    }
    tilemap.set_tile(1,3,[5.0, 0.0, 0.0, 0.0]);
    tilemap.set_tile(2,3,[6.0, 0.0, 0.0, 0.0]);
    tilemap.set_tile(3,3,[7.0, 0.0, 0.0, 0.0]);
    tilemap.set_tile(1,2,[5.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(2,2,[4.0, 0.0, 0.0, 0.0]);
    tilemap.set_tile(3,2,[11.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(1,1,[5.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(2,1,[6.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(3,1,[7.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(1,0,[4.0, 7.0, 0.0, 0.0]);
    tilemap.set_tile(2,0,[4.0, 7.0, 0.0, 0.0]);
    tilemap.set_tile(3,0,[4.0, 7.0, 0.0, 0.0]);
    tilemap.set_tile(4,2,[4.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(5,2,[4.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(6,2,[11.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(4,1,[4.0, 7.0, 0.0, 0.0]);
    tilemap.set_tile(5,1,[4.0, 7.0, 0.0, 0.0]);
    tilemap.set_tile(6,1,[4.0, 7.0, 0.0, 0.0]);
    tilemap.set_tile(6,3,[4.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(6,4,[4.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(6,5,[4.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(6,6,[4.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(6,7,[4.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(5,10,[5.0, 0.0, 0.0, 0.0]);
    tilemap.set_tile(7,10,[7.0, 0.0, 0.0, 0.0]);
    tilemap.set_tile(5,9,[5.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(6,9,[6.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(7,9,[7.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(5,8,[5.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(6,8,[8.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(7,8,[7.0, 2.0, 0.0, 0.0]);
    tilemap.set_tile(5,7,[2.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(7,7,[2.0, 1.0, 0.0, 0.0]);
    tilemap.set_tile(6,10,[2.0, 3.0, 0.0, 0.0]);
    tilemap.set_tile(6,11,[2.0, 2.0, 0.0, 0.0]);
}

impl<R: gfx::Resources> gfx_app::Application<R> for TileMap<R> {
    fn new<F: gfx::Factory<R>>(factory: &mut F, backend: gfx_app::shade::Backend,
           window_targets: gfx_app::WindowTargets<R>) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/tilemap_150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let ps = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/tilemap_150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            .. gfx_app::shade::Source::empty()
        };

        // set up charmap plane and configure its tiles
        let tilemap_size = [24, 24];
        let charmap_size = [16, 16];
        let tile_size = 32;

        let mut tiles = Vec::new();
        for _ in 0 .. tilemap_size[0]*tilemap_size[1] {
            tiles.push(TileMapData::new_empty());
        }

        // TODO: should probably check that charmap is smaller than tilemap
        let mut tm = TileMap {
            tiles: tiles,
            pso: factory.create_pipeline_simple(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap(),
                pipe::new()
                ).unwrap(),
            tilemap_plane: TileMapPlane::new(factory,
                charmap_size[0], charmap_size[1], tile_size,
                window_targets),
            tile_size: tile_size as f32,
            tilemap_size: tilemap_size,
            charmap_size: charmap_size,
            limit_coords: [tilemap_size[0] - charmap_size[0], tilemap_size[1] - charmap_size[1]],
            focus_coords: [0, 0],
            focus_dirty: false,
            input: InputState {
                distance: 800.0,
                x_pos: 0.0,
                y_pos: 0.0,
                move_amt: 10.0,
                offset_amt: 1.0,
            },
        };

        populate_tilemap(&mut tm, tilemap_size);
        tm.set_focus([0, 0]);
        tm
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        // view configuration based on current position
        let view = Matrix4::look_at(
            Point3::new(self.input.x_pos, -self.input.y_pos, self.input.distance),
            Point3::new(self.input.x_pos, -self.input.y_pos, 0.0),
            Vector3::unit_y(),
        );

        self.tilemap_plane.update_view(&view);
        self.tilemap_plane.prepare_buffers(encoder, self.focus_dirty);
        self.focus_dirty = false;

        self.tilemap_plane.clear(encoder);

        encoder.draw(&self.tilemap_plane.slice, &self.pso, &self.tilemap_plane.params);
    }

    fn on(&mut self, event: winit::WindowEvent) {
        if let winit::WindowEvent::KeyboardInput {
            input: winit::KeyboardInput {
                state: winit::ElementState::Pressed,
                virtual_keycode: Some(key),
                .. },
            .. } = event {
            use winit::VirtualKeyCode::*;
            let i = self.input.clone();

            match key {
                // zooming in/out
                Equals => self.input.distance -= i.move_amt,
                Minus | Subtract => self.input.distance += i.move_amt,
                // panning around
                Up => self.input.y_pos -= i.move_amt,
                Down => self.input.y_pos += i.move_amt,
                Left => self.input.x_pos -= i.move_amt,
                Right => self.input.x_pos += i.move_amt,
                W => self.apply_y_offset(i.offset_amt),
                S => self.apply_y_offset(-i.offset_amt),
                D => self.apply_x_offset(i.offset_amt),
                A => self.apply_x_offset(-i.offset_amt),
                _ => ()
            }
        }
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
        self.tilemap_plane.resize(window_targets);
    }
}

pub fn main() {
    use gfx_app::Application;
    let wb = winit::WindowBuilder::new().with_title("Tilemap example");
    TileMap::launch_default(wb);
}
