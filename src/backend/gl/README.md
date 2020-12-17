# gfx-backend-gl

[OpenGL](https://www.khronos.org/opengl/) backend for gfx.

Can only be used on non-Apple Unix systems. The WSI is hard-coded to EGL.

Note: the `Instance`, `Surface`, `PhysicalDevice`, `Device`, and `Queue` can only
have their methods called on the thread where `Instance` was created(!).
Recording command buffers is free-threaded.

## Normalized Coordinates

Render | Depth | Texture
-------|-------|--------
![render_coordinates](../../../info/gl_render_coordinates.png) | ![depth_coordinates](../../../info/gl_depth_coordinates.png) | ![texture_coordinates](../../../info/gl_texture_coordinates.png)

## Binding Model

Dimensions of the model:
  1. Register type: uniform buffers, storage buffers, and combined texture-samplers
  2. Binding slot (0 .. `MAX_COMBINED_TEXTURE_IMAGE_UNITS` for textures)

## GLSL Mirroring

Texture Kind | GLSL sampler
-------------|-------------
`D1` | *g*sampler1D, sampler1DShadow
`D1Array` | *g*sampler1DArray, sampler1DArrayShadow
`D2` | *g*sampler2D, *g*sampler2DMS, sampler2DShadow
`D2Array` | *g*sampler2DArray, *g*sampler2DMSArray, sampler2DArrayShadow
`D3` | *g*sampler3D
`Cube` | *g*samplerCube, samplerCubeShadow
`CubeArray` | *g*samplerCubeArray, samplerCubeArrayShadow

Buffer resource views are seen as *g*samplerBuffer.

Rust basic type | GLSL (1.3 and above)
----------------|---------------------
i32 | int
u32 | uint
f32 | float
f64 | double
