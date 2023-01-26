## Change Log

### v0.18.3
  - `gfx_defines!` uses `#[repr(C)]` for vertex & constant structs. Fixes issues with rust 1.67 field ordering.

### v0.18 (2019-02-12)
  - changed `get_dimensions` to return a minimum of 1
  - Features:
    - raw target clearing ([#2197](https://github.com/gfx-rs/gfx/pull/2197))
    - tessellation with geometry shaders ([#2462](https://github.com/gfx-rs/gfx/pull/2462))
    - GL native texture views ([#2545](https://github.com/gfx-rs/gfx/pull/2545))
    - unsigned uniforms

### v0.17 (2017-12-30)
  - basic WebGL2 support via `wasm32-unknown-emscripten` target ([#1706](https://github.com/gfx-rs/gfx/pull/1706), [#1681](https://github.com/gfx-rs/gfx/pull/1681))
  - `mint` integration instead of `cgmath` ([#1301](https://github.com/gfx-rs/gfx/pull/1301))
  - `draw_state` update to v0.8, breaking the bit flags ([#1708](https://github.com/gfx-rs/gfx/pull/1708))
  - Features:
    - texture to texture copies ([#1544](https://github.com/gfx-rs/gfx/pull/1544), [#1641](https://github.com/gfx-rs/gfx/pull/1641), [#1549](https://github.com/gfx-rs/gfx/pull/1549))
    - DX11 texture readback ([#1630](https://github.com/gfx-rs/gfx/pull/1630))
    - GL mipmap generation ([#1470](https://github.com/gfx-rs/gfx/pull/1470))
    - GL buffer resource views ([#1329](https://github.com/gfx-rs/gfx/pull/1329))
  - Examples:
    - new [render_target](examples/render_target) example
    - screenshot function for [gamma](examples/gamma) example ([#1478](https://github.com/gfx-rs/gfx/pull/1478))

### v0.16 (2017-05-11)
  - `RawGlobal` PSO component ([#1262](https://github.com/gfx-rs/gfx/pull/1262))
  - run-time configurable instance rate ([#1256](https://github.com/gfx-rs/gfx/pull/1256))
  - more convenience traits are derived ([#1249](https://github.com/gfx-rs/gfx/pull/1249))
  - optional cgmath support ([#1242](https://github.com/gfx-rs/gfx/pull/1242))

### v0.15 (2017-04-22)
  - optional serialization support ([#1234](https://github.com/gfx-rs/gfx/pull/1234))
  - better GL state caching ([#1221](https://github.com/gfx-rs/gfx/pull/1221))
  - GL texture staging ([#1202](https://github.com/gfx-rs/gfx/pull/1202))
  - primitives with adjacency ([#1154](https://github.com/gfx-rs/gfx/pull/1154))
  - metal backend improvements ([#1165](https://github.com/gfx-rs/gfx/pull/1165), [#1175](https://github.com/gfx-rs/gfx/pull/1175))
  - resource mapping improvements

### v0.14 (2017-01-16)
  - fixed `Fence` and `Sync` bounds ([#1095](https://github.com/gfx-rs/gfx/pull/1095))
  - dx11 buffer mapping support ([#1099](https://github.com/gfx-rs/gfx/pull/1099), [#1105](https://github.com/gfx-rs/gfx/pull/1105))
  - redesigned resource usage model for next-gen compatibility ([#1123](https://github.com/gfx-rs/gfx/pull/1123))
  - buffer copy support ([#1129](https://github.com/gfx-rs/gfx/pull/1129))
  - fixed and improved some errors ([#1137](https://github.com/gfx-rs/gfx/pull/1137), [#1138](https://github.com/gfx-rs/gfx/pull/1138))
  - application launcher revamp and resize support ([#1121](https://github.com/gfx-rs/gfx/pull/1121))

### v0.13 (2016-12-18)
  - experimental Metal backend ([#969](https://github.com/gfx-rs/gfx/pull/969), [#1049](https://github.com/gfx-rs/gfx/pull/1049), [#1050](https://github.com/gfx-rs/gfx/pull/1050))
  - persistent mapping ([#1026](https://github.com/gfx-rs/gfx/pull/1026))
  - tessellation support ([#1027](https://github.com/gfx-rs/gfx/pull/1027), [#1088](https://github.com/gfx-rs/gfx/pull/1088))
  - new examples: gamma, particle, terrain_tessellated
  - better PSO error messages, constant offset checks ([#1004](https://github.com/gfx-rs/gfx/pull/1004))
  - unified scissor: now Y-reversed on GL ([#1092](https://github.com/gfx-rs/gfx/pull/1092))
  - `const` resources are now called `immutable`
  - faster handle clones and cleaner core  API ([#1031](https://github.com/gfx-rs/gfx/pull/1031))

### v0.12 (2016-06-23)
  - Android / GLES support ([#993](https://github.com/gfx-rs/gfx/pull/993))
  - GL unsigned int samplers ([#991](https://github.com/gfx-rs/gfx/pull/991))
  - better errors ([#976](https://github.com/gfx-rs/gfx/pull/976))
  - better GLSL pre core reflection

### v0.11 (2016-04-30)
  - modified `Slice` API ([#955](https://github.com/gfx-rs/gfx/pull/955))
  - fixed GL blending where it's not in the core ([#953](https://github.com/gfx-rs/gfx/pull/953))
  - raw PSO components for vertex buffers and render targets

### v0.10.2 (2016-04-15)
  - fixed get_texel_count ([#937](https://github.com/gfx-rs/gfx/pull/937))

### v0.10.1 (2016-03-26)
  - fixed update_texture ([#912](https://github.com/gfx-rs/gfx/pull/912))

### v0.10 (2016-03-21)
  - Direct3D 11 backend ([#861](https://github.com/gfx-rs/gfx/pull/861))

### v0.9.2 (2016-02-24)
  - fixed universal format views ([#886](https://github.com/gfx-rs/gfx/pull/886))
  - fixed constant buffer binding ([#828](https://github.com/gfx-rs/gfx/pull/828))

### v0.9.1 (2016-02-19)
  - window resize support ([#879](https://github.com/gfx-rs/gfx/pull/879))
  - deriving windows attributes from target formats ([#874](https://github.com/gfx-rs/gfx/pull/874))

### v0.9 (2016-01-22)
  - Pipepeline State Object revolution ([#828](https://github.com/gfx-rs/gfx/pull/828))
