# gfx-rs

[![Build Status](https://travis-ci.org/gfx-rs/gfx-rs.png?branch=master)](https://travis-ci.org/gfx-rs/gfx-rs)
[![Gitter Chat](https://badges.gitter.im/gfx-rs/gfx-rs.png)](https://gitter.im/gfx-rs/gfx-rs)
[![Stories in Ready](https://badge.waffle.io/gfx-rs/gfx-rs.png?label=S-ready&title=issues)](https://waffle.io/gfx-rs/gfx-rs)

A lightweight buffer, shader and render queue manager for Rust.

## Getting started

Add the following to your `Cargo.toml`:

```toml
[dependencies.gfx]
git = "http://github.com/gfx-rs/gfx-rs"
```

To use [gl-init](https://github.com/tomaka/gl-init-rs/) with `gfx`, also add:

```toml
[dependencies.gl_init_platform]
git = "http://github.com/gfx-rs/gfx-rs"
```

To use [glfw](https://github.com/bjz/glfw-rs/) with `gfx`, also add:

```toml
[dependencies.glfw_platform]
git = "http://github.com/gfx-rs/gfx-rs"
```

See the [triangle example](./src/examples/triangle) for an example using both.

## Building the examples

~~~sh
make -C src/examples
~~~

## The Problem

- Graphics APIs are difficult and diverse in nature. We've seen Mantle and
  Metal popping out of nowhere. Even for OpenGL there are different profiles
  that may need to be supported.
- Communicating with the driver is considered expensive, thus feeding it should
  be done in parallel with the user code.
- Graphics programming is dangerous. Using Rust allows building a safer
  abstraction without run-time overhead.

## Design Goals

- Safe but non-limiting higher level interface
- Simple, lightweight implementation
- Low performance overhead
- Graphics API agnostic (OpenGL/Direct3D/Metal)
- Maths library agnostic
- Composable (a library, not a framework)
- Compatible with Rust's task-based concurrency model
- Clear documentation with examples

## Possible Solutions

- Verify compatibility of the shader inputs with user-provided data.
- Use Rust procedural macros to generate the code for querying and uploading
  of shader parameters.
- Make use of 'draw call bucketing'. See [research.md](wiki/research.md) for more information.
- Leave scene and model management up to the client, and focus instead on
  buffers and shaders.
- Provide structural data types (as opposed to nominal ones) in order to make
  interfacing with other maths libraries easier. For example:
~~~rust
pub type Vertex4<T> = [T,..4];
pub type Matrix4x3<T> = [[T,..3],..4];
~~~

## Note

gfx-rs is still in the early stages of development. Help is most appreciated.

If you are interested in helping out, you can contact the developers on
[Gitter](https://gitter.im/gfx-rs/gfx-rs). See [contrib.md](wiki/contrib.md) for contant
information and contribution guidelines.
