# gfx-backend-webgpu

[WebGPU](https://gpuweb.github.io/gpuweb/) backend for gfx-rs.

Currently requires to be compiled with the `web_sys_unstable_apis` flag.

```sh
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --target wasm32-unknown-unknown
```

## Binding Model

Dimensions of the model:
  1. Shader stage: vs, fs, cs
  2. Resource group: 0 .. 4
  3. Binding: semi-sparse

## Normalized Coordinates

Render | Depth | Texture
-------|-------|--------

TODO
