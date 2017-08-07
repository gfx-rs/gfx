@echo off
set FXC="%DXSDK_DIR%\Utilities\bin\x64\fxc.exe" -nologo
if not exist data mkdir data
%FXC% /T vs_4_0 /E Vertex /Fo data/vertex.fx shader/terrain.hlsl
%FXC% /T hs_5_0 /E HS /Fo data/hull.fx shader/terrain.hlsl
%FXC% /T ds_5_0 /E DS /Fo data/domain.fx shader/terrain.hlsl
%FXC% /T ps_4_0 /E Pixel /Fo data/pixel.fx shader/terrain.hlsl
