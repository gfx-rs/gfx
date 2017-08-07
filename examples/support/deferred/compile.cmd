@echo off
set FXC="%DXSDK_DIR%\Utilities\bin\x64\fxc.exe" -nologo
if not exist data mkdir data
%FXC% /T vs_4_0 /E TerrainVs /Fo data/terrain_vs.fx shader/deferred.hlsl
%FXC% /T ps_4_0 /E TerrainPs /Fo data/terrain_ps.fx shader/deferred.hlsl
%FXC% /T vs_4_0 /E BlitVs /Fo data/blit_vs.fx shader/deferred.hlsl
%FXC% /T ps_4_0 /E BlitPs /Fo data/blit_ps.fx shader/deferred.hlsl
%FXC% /T vs_4_0 /E LightVs /Fo data/light_vs.fx shader/deferred.hlsl
%FXC% /T ps_4_0 /E LightPs /Fo data/light_ps.fx shader/deferred.hlsl
%FXC% /T vs_4_0 /E EmitterVs /Fo data/emitter_vs.fx shader/deferred.hlsl
%FXC% /T ps_4_0 /E EmitterPs /Fo data/emitter_ps.fx shader/deferred.hlsl
