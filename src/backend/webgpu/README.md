# gfx-backend-webgpu

[WebGPU](https://gpuweb.github.io/gpuweb/) backend for gfx-rs.

Currently requires to be compiled with the `web_sys_unstable_apis` flag.

```sh
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --target wasm32-unknown-unknown
```

## Normalized Coordinates

Render | Depth | Texture
-------|-------|--------

TODO
