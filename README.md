<!--
    Copyright 2014 The Gfx-rs Developers.

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
-->

# gfx-rs

[![Build Status](https://travis-ci.org/gfx-rs/gfx-rs.png?branch=master)](https://travis-ci.org/gfx-rs/gfx-rs)
[![Gitter Chat](https://badges.gitter.im/gfx-rs/gfx-rs.png)](https://gitter.im/gfx-rs/gfx-rs)
[![Stories in Ready](https://badge.waffle.io/gfx-rs/gfx-rs.png?label=S-ready&title=issues)](https://waffle.io/gfx-rs/gfx-rs)
[Documentation](http://gfx-rs.github.io/gfx-rs/gfx/index.html)

`gfx-rs` is a high-performance, bindless graphics API for the Rust
programming language. It aims to be the default API for Rust graphics: for
one-off applications, or higher level libraries or engines.

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
- procedural content generation ([genmesh](http://github.com/gfx-rs/genmesh))
- lighting and shadows ([gfx_pipeline](http:://github.com/kvark/gfx_pipeline))
- visibility determination ([gfx_scene](http:://github.com/gfx-rs/gfx_scene))
- draw call reordering
- de-serializing of scene data formats
- abstractions for platform-specific shaders
- material abstractions ([gfx_pipeline](http:://github.com/kvark/gfx_pipeline))

## Getting started

Use gfx [package](https://crates.io/crates/gfx) from `crates.io`.

See the [triangle example](examples/triangle) for a typical context
initialization with [glutin](https://github.com/tomaka/glutin/), or
[cube example](examples/cube) for [glfw](https://github.com/bjz/glfw-rs/).

If you are interested in helping out, or just need to talk to someone,
please contact the developers on [Gitter](https://gitter.im/gfx-rs/gfx-rs).
