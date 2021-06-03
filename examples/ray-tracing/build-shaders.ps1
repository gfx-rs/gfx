$shaders = @(
    "$PSScriptRoot\data\simple.rchit"
    "$PSScriptRoot\data\simple.rgen"
    "$PSScriptRoot\data\simple.rmiss"
)

Remove-Item $PSScriptRoot\data\*.spv

foreach ($shader in $shaders) {
    & glslangValidator --target-env vulkan1.2 --entry-point main $shader -o "$shader.spv"
}
