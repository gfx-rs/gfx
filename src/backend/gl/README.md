# gfx_device_gl

[OpenGL](https://www.khronos.org/opengl/) backend for gfx.

## Normalized Coordinates

Render | Depth | Texture
-------|-------|--------
![render_coordinates](../../../info/gl_render_coordinates.png) | ![depth_coordinates](../../../info/gl_depth_coordinates.png) | ![texture_coordinates](../../../info/gl_texture_coordinates.png)

## GLSL Mirroring

PSO component | GLSL component
--------------|----------------
`Vertex/InstanceBuffer` | a collection of vertex shader inputs
`ConstantBuffer` | [Uniform Buffer Object](https://www.opengl.org/wiki/Uniform_Buffer_Object)
`Global` | [Uniform](https://www.opengl.org/wiki/Uniform_(GLSL))
`Render/BlendTarget` | fragment shader output
`Depth/StencilTarget` | [depth](https://www.opengl.org/wiki/Depth_Test), [stencil](https://www.opengl.org/wiki/Stencil_Test)
`UnorderedAccess` | TODO
`Scissor` | not visible
`BlendRef` | not visible

`TextureSampler`s correspond to the following [GLSL samplers](https://www.opengl.org/wiki/Sampler_(GLSL)), when you see a *g* preceding a sampler name, it represents any of the 3 possible prefixes (nothing for float, i for signed integer, and u for unsigned integer):

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
