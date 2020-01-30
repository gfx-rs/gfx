# gfx_window_glutin

Glutin window backend for gfx-rs

## Usage

Make sure you have the following in your `Cargo.toml`:

```toml
gfx_core = "0.9"
gfx_device_gl = "0.16"
gfx_window_glutin = "0.32"
glutin = "0.22"
```

Then, initialize `gfx` as follows:

```rust
extern crate gfx_core;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx_core::format::{DepthStencil, Rgba8};

fn main() {
    let event_loop = EventLoop::new();
    let window_builder = WindowBuilder::new().with_title("Example".to_owned());
    let context = ContextBuilder::new();
    let (window, device, factory, rtv, stv) =
        gfx_window_glutin::init::<Rgba8, DepthStencil, _>(window_builder, context, &event_loop);

    // your code
}
```
