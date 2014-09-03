# gfx-rs

[![Build Status](https://travis-ci.org/gfx-rs/gfx-rs.png?branch=master)](https://travis-ci.org/gfx-rs/gfx-rs)
[![Gitter Chat](https://badges.gitter.im/gfx-rs/gfx-rs.png)](https://gitter.im/gfx-rs/gfx-rs)
[![Stories in Ready](https://badge.waffle.io/gfx-rs/gfx-rs.png?label=S-ready&title=issues)](https://waffle.io/gfx-rs/gfx-rs)

`gfx-rs` is a high-performance, bindless graphics API for the Rust
programming language. It aims to be the default API for Rust graphics: for
one-off applications, or higher level libraries or engines.

The API provides a common abstraction over various GPU features, including:

- shaders
- shader parameters
- vertex and index buffers
- vertex attributes
- textures
- color masks
- scissor masks
- stencil masks
- depth tests
- blend functions
- multisampling
- draw instancing

## Why gfx-rs?

- Graphics APIs are mostly designed with C and C++ in mind, and hence are
  dangerous and error prone, with little static safety guarantees.
- Providing type safe wrappers around platform-specific APIs is feasible, but
  only pushes the problem of platform independence to a higher level of
  abstraction, often to the game or rendering engine.
- Modern graphics APIs, whilst providing a great degree of flexibility and a
  high level of performance, often have a much higher barrier to entry than
  traditional [fixed-function](http://en.wikipedia.org/wiki/Fixed-function) APIs.
- Graphics APIs like OpenGL still [require the developer to 'bind' and 'unbind'
  objects](http://www.arcsynthesis.org/gltut/Basics/Intro%20What%20is%20OpenGL.html#d0e887)
  in order to perform operations on them. This results in a large amount of
  boiler plate code, and brings with it the usual problems associated with
  global state.

## Goals

`gfx-rs` aims to be:

- type-safe and memory-safe
- compatible with Rust's concurrency model
- highly performant with minimal latency
- an abstraction over multiple graphics APIs: OpenGL, Direct3D, Mantle, etc.
- orthogonal to context backends: GLFW, SDL2, gl-init-rs, etc.

## Non-goals

`gfx-rs` is not:

- a rendering engine
- a game engine
- bound to a specific maths library

`gfx-rs` will not handle:

- window and input management
- mathematics and transformations
- lighting and shadows
- visibility determination
- draw call reordering
- de-serializing of scene data formats
- abstractions for platform-specific shaders
- material abstractions

## Getting started

Add the following to your `Cargo.toml`:

~~~toml
[dependencies.gfx]
git = "http://github.com/gfx-rs/gfx-rs"
~~~

See the [triangle example](./src/examples/triangle) for a typical context
initialization with [glfw](https://github.com/bjz/glfw-rs/), or
[gl-init example](./src/examples/gl-init) for [gl-init](https://github.com/tomaka/gl-init-rs/).

## Crate heirachy

![Dependency graph](diagrams/png/dependencies.png)

## Building the examples

~~~sh
make -C examples
~~~

## Note

gfx-rs is still in the early stages of development. Help is most appreciated.

If you are interested in helping out, you can contact the developers on
[Gitter](https://gitter.im/gfx-rs/gfx-rs). See [contrib.md](wiki/contrib.md) for
contant information and contribution guidelines.
