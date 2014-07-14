# gfx-rs

[![Build Status](https://travis-ci.org/bjz/gfx-rs.png?branch=master)](https://travis-ci.org/bjz/gfx-rs)
[![Gitter chat](https://badges.gitter.im/bjz/gfx-rs.png)](https://gitter.im/bjz/gfx-rs)

A lightweight buffer, shader and render queue manager for Rust.

## Getting Started

For the initial build of both gfx-rs and all its dependencies, use this incantation:

~~~sh
make init
~~~

Afterwards, you can use these targets:

~~~sh
make                    # build libraries, examples, and documentation
make lib                # build libraries only
make examples           # build the examples
make test               # build and run the unit tests
make doc                # generate documentation
make clean              # clean up everything (except for the dependencies)

make submodule-update   # initialise and update submodules
make deps               # build the dependencies
make clean-deps         # clean up the dependencies
~~~

## The Problem

- Graphics APIs (Especially OpenGL), require the liberal use of unsafe
  operations and non-ideomatic code
- Moving large amounts of data around is expensive
- Syncing draw calls across tasks is difficult
- Draw calls are most efficient when performed in bulk
- Many rendering engines are sprawling frameworks that require applications to
  be tightly coupled with their abstractions

## Design Goals

- Simple, lightweight implementation
- Low performance overhead
- Graphics API agnostic (OpenGL/DirectX)
- Maths library agnostic
- Composable (a library, not a framework)
- Compatible with Rust's task-based concurrency model
- Clear documentation with examples

## Possible Solutions

- Use a handle-based API to manage buffer and shader objects. This would allow
  data to be packed in arrays as opposed to being distributed across
  tree-based struct hierarchies. It would also make batch processing easier.
  See `research.md` for more information on this data model. One issue with
  this model could be the problem of 'handle lifetimes' - ie. what happens if
  the data associated with a handle is removed? Using this model could negate
  some of the advantages of using Rust in the first place.
- Make use of 'draw call bucketing'. See `research.md` for more information.
- Leave scene and model management up to the client, and focus instead on
  buffers and shaders. Provide ways of accessing the underlying rendering API,
  to allow clients to make use of advanced, non-standard features if necessary.
- Provide structural data types (as opposed to nominal ones) in order to make
  interfacing with other maths libraries easier. For example:

~~~rust
pub type Vertex4<T> = [T,..4];
pub type Matrix4x3<T> = [[T,..3],..4];
~~~

## Note

gfx-rs is still in the early stages of development. Help is most appreciated.

If you are interested in helping out, you can contact the developers on
[Gitter](https://gitter.im/bjz/gfx-rs). They are also often reachable on
`irc.mozilla.org #rust-gamedev`.
