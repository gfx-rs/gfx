# gfx_device_gl

[OpenGL](https://www.khronos.org/opengl/) backend for gfx.

## GLSL Mirroring

PSO Component | GLSL component
--------------|----------------
`Vertex/InstanceBuffer` | a collection of vertex shader inputs
`ConstantBuffer` | [Uniform Buffer Object](https://www.opengl.org/wiki/Uniform_Buffer_Object)
`Global` | [Uniform](https://www.opengl.org/wiki/Uniform_(GLSL))
`Render/BlendTarget` | fragment shader output
`Depth/StencilTarget` | [depth](https://www.opengl.org/wiki/Depth_Test), [stencil](https://www.opengl.org/wiki/Stencil_Test)
`UnorderedAccess` | TODO
`Scissor` | not visible
`BlendRef` | not visible

`TextureSampler`'s are accessible with the following  [GLSL samplers](https://www.opengl.org/wiki/Sampler_(GLSL)):

Texture type | GLSL sampler
-------------|-------------
Texture | sampler *Kind*
Kind::D1 | sampler1D
Kind::D1Array | sampler1DArray
Kind::D2 | sampler2D, sampler2DMS
Kind::D2Array | sampler2DArray, sampler2DMSArray
Kind::D3 | sampler3D
Kind::Cube | samplerCube
Kind::CubeArray | samplerCubeArray
Buffer | samplerBuffer
Depth/Stencil | sampler *Kind* Shadow

Rust basic type | GLSL (1.3 and above)
----------------|---------------------
i32 | int
u32 | uint
f32 | float
f64 | double
