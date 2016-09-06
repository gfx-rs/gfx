#!/bin/sh

echo Compiling...
glslangValidator -V -o ../data/vert.spv cube_vulkan.glsl.vert
glslangValidator -V -o ../data/frag.spv cube_vulkan.glsl.frag
echo Validating...
spirv-val ../data/vert.spv
spirv-val ../data/frag.spv
