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
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate rand;
extern crate genmesh;
extern crate noise;
extern crate image;

use std::collections::HashMap;
use std::io::Cursor;

use glutin::{PollEventsIterator, Event, VirtualKeyCode, ElementState};

pub use gfx::format::{DepthStencil, Rgba8};
use gfx::traits::{FactoryExt};

use cgmath::FixedArray;
use cgmath::{Matrix4, AffineMatrix3};
use cgmath::{Point3, Vector3};
use cgmath::{Transform};

use genmesh::{Vertices, Triangulate};
use genmesh::generators::{Plane, SharedVertex, IndexedPolygon};

// this is a value based on a max buffer size (and hence tilemap size) of 64x64
// I imagine you would have a max buffer length, with multiple TileMap instances
// of varying sizes based on current screen resolution
pub const TILEMAP_BUF_LENGTH: usize = 4096;

// texture loading boilerplate
pub fn load_texture<R, F>(factory: &mut F, data: &[u8])
                    -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String>
        where R: gfx::Resources, F: gfx::Factory<R> {
    use gfx::tex as t;
    let img = image::load(Cursor::new(data), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = t::Kind::D2(width as t::Size, height as t::Size, t::AaMode::Single);
    let (_, view) = factory.create_texture_const::<Rgba8>(kind, gfx::cast_slice(&img), false).unwrap();
    Ok(view)
}

// this abstraction is provided to get a slightly better API around
// input handling
pub struct InputHandler {
    key_map: HashMap<VirtualKeyCode, bool>,
    key_list: Vec<VirtualKeyCode>
}

impl InputHandler {
    pub fn new() -> InputHandler {
        InputHandler {
            key_map: HashMap::new(),
            key_list: Vec::new()
        }
    }
    pub fn update(& mut self, events: PollEventsIterator) {
        for event in events {
            match event {
                Event::KeyboardInput(ElementState::Pressed, _, key_opt) => {
                    let pressed_key = key_opt.unwrap();
                    if self.key_map.contains_key(&pressed_key) {
                        self.key_map.insert(pressed_key, true);
                    } else {
                        println!("unknown key {:?} pressed", key_opt);
                    }
                },
                Event::KeyboardInput(ElementState::Released, _, key_opt) => {
                    let released_key = key_opt.unwrap();
                    if self.key_map.contains_key(&released_key) {
                        self.key_map.insert(released_key, false);
                    }
                },
                _ => {}
            }
        }
    }
    pub fn watch(&mut self, key: VirtualKeyCode) {
        if self.key_map.contains_key(&key) {
            panic!("watching key that is already tracked");
        }
        self.key_map.insert(key, false);
        self.key_list.push(key);
    }
    pub fn is_pressed(&self, key: VirtualKeyCode) -> bool {
        if self.key_map.contains_key(&key) == false {
            panic!("checking keydown for key that isn't being tracked");
        }
        *self.key_map.get(&key).unwrap()
    }
}

// Actual tilemap data that makes up the elements of the UBO.
// NOTE: It may be a bug, but it appears that
// [f32;2] won't work as UBO data. Possibly an issue with
// binding generation
gfx_constant_struct!(TileMapData {
    data: [f32; 4],
});

impl TileMapData {
    pub fn new_empty() -> TileMapData {
        TileMapData { data: [0.0, 0.0, 0.0, 0.0] }
    }
    pub fn new(data: [f32; 4]) -> TileMapData {
        TileMapData { data: data }
    }
}

// Vertex data
gfx_vertex_struct!( VertexData {
    pos: [f32; 3] = "a_Pos",
    buf_pos: [f32; 2] = "a_BufPos",
});

// Pipeline state definition
gfx_pipeline!(pipe {
    vbuf: gfx::VertexBuffer<VertexData> = (),
    // projection stuff
    model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
    view: gfx::Global<[[f32; 4]; 4]> = "u_View",
    proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
    // tilemap stuff
    tilesheet: gfx::TextureSampler<[f32; 4]> = "t_TileSheet",
    tilemap: gfx::ConstantBuffer<TileMapData> = "b_TileMap",
    world_size: gfx::Global<[f32; 3]> = "u_WorldSize",
    tilesheet_size: gfx::Global<[f32; 4]> = "u_TilesheetSize",
    offsets: gfx::Global<[f32; 2]> = "u_TileOffsets",
    // output
    out_color: gfx::RenderTarget<Rgba8> = "o_Color",
    out_depth: gfx::DepthTarget<DepthStencil> =
        gfx::preset::depth::LESS_EQUAL_WRITE,
});

// Abstracts the plane mesh and uniform data
// Also holds a Vec<TileMapData> as a working data
// set for consumers
pub struct TileMapPlane<R> where R: gfx::Resources {
    pub params: pipe::Data<R>,
    pub slice: gfx::Slice<R>,
    pub data: Vec<TileMapData>,
}

impl<R> TileMapPlane<R> where R: gfx::Resources {
    pub fn new<F>(factory: &mut F, width: usize, height: usize, tile_size: usize,
                  main_color: &gfx::handle::RenderTargetView<R, Rgba8>,
                  main_depth: &gfx::handle::DepthStencilView<R, DepthStencil>,
                  aspect_ratio: f32)
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

        // law out the vertices of the plane mesh based on the configured tile size information,
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

        let (vbuf, slice) = factory.create_vertex_buffer_indexed(&vertex_data, &index_data[..]);

        let tile_texture = load_texture(factory, tilesheet_bytes).unwrap();
        let tilemap_buf = factory.create_constant_buffer(TILEMAP_BUF_LENGTH);

        let params = pipe::Data {
            vbuf: vbuf,
            model: Matrix4::identity().into_fixed(),
            view: Matrix4::identity().into_fixed(),
            proj: cgmath::perspective(cgmath::deg(60.0f32), aspect_ratio, 0.1, 4000.0).into_fixed(),
            tilesheet: (tile_texture, factory.create_sampler_linear()),
            tilemap: tilemap_buf,
            world_size: [width as f32, height as f32, tile_size as f32],
            tilesheet_size: [tilesheet_width as f32, tilesheet_height as f32, tilesheet_total_width as f32, tilesheet_total_height as f32],
            offsets: [0.0, 0.0],
            out_color: main_color.clone(),
            out_depth: main_depth.clone(),
        };

        let mut charmap_data = Vec::with_capacity(total_size);
        for _ in 0..total_size {
            charmap_data.push(TileMapData::new_empty());
        }

        TileMapPlane {
            slice: slice,
            params: params,
            data: charmap_data,
        }
    }

    pub fn update_data<F>(&mut self, factory: &mut F) where F: gfx::Factory<R> {
        factory.update_buffer(&self.params.tilemap, &self.data, 0).unwrap();
    }
    pub fn update_view(&mut self, view: &AffineMatrix3<f32>) {
        self.params.view = view.mat.into_fixed();
    }
    pub fn update_x_offset(&mut self, amt: f32) {
        self.params.offsets[0] = amt;
    }
    pub fn update_y_offset(&mut self, amt: f32) {
        self.params.offsets[1] = amt;
    }
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
}

impl<R: gfx::Resources> TileMap<R> {
    pub fn new<F>(factory: &mut F, tilemap_size: [usize; 2], charmap_size: [usize; 2], tile_size: usize,
                  main_color: &gfx::handle::RenderTargetView<R, Rgba8>,
                  main_depth: &gfx::handle::DepthStencilView<R, DepthStencil>,
                  aspect_ratio: f32)
                  -> TileMap<R> where F: gfx::Factory<R> {
        let mut tiles = Vec::new();
        for _ in 0 .. tilemap_size[0]*tilemap_size[1] {
            tiles.push(TileMapData::new_empty());
        }
        let pso = factory.create_pipeline_simple(
            include_bytes!("tilemap_150.glslv"),
            include_bytes!("tilemap_150.glslf"),
            gfx::state::CullFace::Back,
            pipe::new()
            ).unwrap();

        // TODO: should probably check that charmap is smaller than tilemap
        TileMap {
            tiles: tiles,
            pso: pso,
            tilemap_plane: TileMapPlane::new(factory,
                charmap_size[0], charmap_size[1], tile_size,
                main_color, main_depth, aspect_ratio),
            tile_size: tile_size as f32,
            tilemap_size: tilemap_size,
            charmap_size: charmap_size,
            limit_coords: [tilemap_size[0] - charmap_size[0], tilemap_size[1] - charmap_size[1]],
            focus_coords: [0,0]
        }
    }
    pub fn set_focus<F>(&mut self, factory: &mut F, focus: [usize; 2]) where F: gfx::Factory<R> {
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
            self.tilemap_plane.update_data(factory);
        } else {
            panic!("tried to set focus to {:?} with tilemap_size of {:?}", focus, self.tilemap_size);
        }
    }
    pub fn apply_x_offset<F>(&mut self, factory: &mut F, offset_amt: f32) where F: gfx::Factory<R> {
        let mut new_offset = self.tilemap_plane.params.offsets[0] + offset_amt;
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
            self.set_focus(factory, [new_x, curr_focus[1]]);
        }
        self.tilemap_plane.update_x_offset(new_offset);
    }
    pub fn apply_y_offset<F>(&mut self, factory: &mut F, offset_amt: f32) where F: gfx::Factory<R> {
        let mut new_offset = self.tilemap_plane.params.offsets[1] + offset_amt;
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
            self.set_focus(factory, [curr_focus[0], new_y]);
        }
        self.tilemap_plane.update_y_offset(new_offset);
    }
    pub fn update<C>(&mut self, view: &AffineMatrix3<f32>, encoder: &mut gfx::Encoder<R, C>)
            where C: gfx::CommandBuffer<R> {
        self.tilemap_plane.update_view(view);
        encoder.draw(&self.tilemap_plane.slice, &self.pso, &self.tilemap_plane.params);
    }
    fn calc_idx(&self, xpos: usize, ypos: usize) -> usize {
        (ypos * self.tilemap_size[0]) + xpos
    }
    pub fn set_tile(&mut self, xpos: usize, ypos: usize, data: [f32; 4]) {
        let idx = self.calc_idx(xpos, ypos);
        self.tiles[idx] = TileMapData::new(data);
    }
}


pub fn populate_tilemap<R>(tilemap: &mut TileMap<R>, tilemap_size: [usize; 2]) where R: gfx::Resources {
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

pub fn main() {
    use gfx::{Device};

    let builder = glutin::WindowBuilder::new()
        .with_title("Tilemap example".to_string());
    let (window, mut device, mut factory, main_color, main_depth) =
        gfx_window_glutin::init::<Rgba8>(builder);
    let mut encoder = factory.create_encoder();

    // clear window contents
    encoder.clear(&main_color, [0.0, 0.0, 0.0, 1.0]);
    device.submit(encoder.as_buffer());
    window.swap_buffers().unwrap();

    // set up charmap plane and configure its tiles
    let tilemap_size = [24, 24];
    let aspect_ratio = {
        let (w, h) = window.get_inner_size().unwrap();
        w as f32 / h as f32
    };
    let mut tilemap = TileMap::new(&mut factory,
        tilemap_size, [16, 16], 32,
        &main_color, &main_depth, aspect_ratio);
    populate_tilemap(&mut tilemap, tilemap_size);

    tilemap.set_focus(&mut factory, [0,0]);

    // reusable variables for camera position
    let mut distance = 800.0;
    let mut x_pos = 0.0;
    let mut y_pos = 0.0;
    let move_amt = 10.0;
    let offset_amt = 1.0;
    // input handling
    let mut handler = InputHandler::new();
    handler.watch(glutin::VirtualKeyCode::Escape);
    handler.watch(glutin::VirtualKeyCode::Up);
    handler.watch(glutin::VirtualKeyCode::Down);
    handler.watch(glutin::VirtualKeyCode::Left);
    handler.watch(glutin::VirtualKeyCode::Right);
    handler.watch(glutin::VirtualKeyCode::Equals);
    handler.watch(glutin::VirtualKeyCode::Minus);
    handler.watch(glutin::VirtualKeyCode::W);
    handler.watch(glutin::VirtualKeyCode::S);
    handler.watch(glutin::VirtualKeyCode::A);
    handler.watch(glutin::VirtualKeyCode::D);
    'main: loop {
        // input handler
        handler.update(window.poll_events());
        // quit when Esc is pressed.
        if handler.is_pressed(glutin::VirtualKeyCode::Escape) {
            break 'main;
        }
        // zooming in/out
        if handler.is_pressed(glutin::VirtualKeyCode::Equals) {
            distance -= move_amt;
        }
        if handler.is_pressed(glutin::VirtualKeyCode::Minus) {
            distance += move_amt;
        }
        // panning around
        if handler.is_pressed(glutin::VirtualKeyCode::Up) {
            y_pos -= move_amt;
        }
        if handler.is_pressed(glutin::VirtualKeyCode::Down) {
            y_pos += move_amt;
        }
        if handler.is_pressed(glutin::VirtualKeyCode::Left) {
            x_pos -= move_amt;
        }
        if handler.is_pressed(glutin::VirtualKeyCode::Right) {
            x_pos += move_amt;
        }
        if handler.is_pressed(glutin::VirtualKeyCode::W) {
            tilemap.apply_y_offset(&mut factory, offset_amt);
        }
        if handler.is_pressed(glutin::VirtualKeyCode::S) {
            tilemap.apply_y_offset(&mut factory, -offset_amt);
        }
        if handler.is_pressed(glutin::VirtualKeyCode::D) {
            tilemap.apply_x_offset(&mut factory, offset_amt);
        }
        if handler.is_pressed(glutin::VirtualKeyCode::A) {
            tilemap.apply_x_offset(&mut factory, -offset_amt);
        }

        // view configuration based on current position
        let view: AffineMatrix3<f32> = Transform::look_at(
            &Point3::new(x_pos, -y_pos, distance),
            &Point3::new(x_pos, -y_pos, 0.0),
            &Vector3::unit_y(),
        );

        encoder.reset();
        encoder.clear(&main_color,
            [16.0 / 256.0, 14.0 / 256.0, 22.0 / 256.0, 1.0]);
        encoder.clear_depth(&main_depth, 1.0);

        tilemap.update(&view, &mut encoder);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
