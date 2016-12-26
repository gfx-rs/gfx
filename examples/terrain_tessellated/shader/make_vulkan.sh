#!/bin/sh

echo Compiling...
ln -s terrain_150.glslv out.vert
ln -s terrain_150.glslf out.frag
glslangValidator -V -o ../data/vert.spv out.vert
glslangValidator -V -o ../data/frag.spv out.frag
rm out.vert out.frag
echo Validating...
spirv-val ../data/vert.spv
spirv-val ../data/frag.spv
