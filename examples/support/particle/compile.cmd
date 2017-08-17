@echo off
set FXC="%DXSDK_DIR%\Utilities\bin\x64\fxc.exe" -nologo
if not exist data mkdir data
%FXC% /T vs_4_0 /E VS /Fo data/vs_particle.fx shader/particle.hlsl
%FXC% /T gs_4_0 /E GS /Fo data/gs_particle.fx shader/particle.hlsl
%FXC% /T ps_4_0 /E PS /Fo data/ps_particle.fx shader/particle.hlsl
