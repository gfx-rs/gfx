# Cube Example

A simple example showing how to render a textured cube using vertex and index
buffers, GLSL shaders, and uniform parameters. It is also using cgmath-rs to
compute the view-projection matrix.

The example provides two versions of each shader: for GLSL 1.20 and 1.50-core.
This is needed for proper OSX compatibility and ensures it can run on any
system.

## Screenshot

![Cube Example](screenshot.png)
