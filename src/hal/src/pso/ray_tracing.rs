//! Ray tracing pipeline descriptor.

use crate::{
    buffer::{Offset, Stride},
    pso::{PipelineCreationFlags, ShaderStageFlags},
    Backend,
};

use super::{BasePipeline, EntryPoint};

/// TODO docs
pub const SHADER_UNUSED: u32 = !0;

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkStridedDeviceAddressRegionKHR.html
#[derive(Debug)]
pub struct ShaderBindingTable<'a, B: Backend> {
    /// TODO docs
    pub buffer: &'a B::Buffer,
    /// TODO docs
    pub offset: Offset,
    /// TODO docs
    pub stride: Stride,
    /// TODO docs
    pub size: u64,
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkShaderGroupShaderKHR.html
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GroupShader {
    /// TODO docs
    General,
    /// TODO docs
    ClosestHit,
    /// TODO docs
    AnyHit,
    /// TODO docs
    Intersection,
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingShaderGroupTypeKHR.html
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GroupType {
    /// TODO docs
    General,
    /// TODO docs
    TrianglesHitGroup,
    /// TODO docs
    ProceduralHitGroup,
}

/// A description of the data needed to construct a ray tracing pipeline.
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingPipelineCreateInfoKHR.html
#[derive(Debug)]
pub struct RayTracingPipelineDesc<'a, B: Backend> {
    /// TODO docs
    pub flags: PipelineCreationFlags,

    /// TODO docs
    pub stages: &'a [(ShaderStageFlags, EntryPoint<'a, B>)],

    /// TODO docs
    pub groups: &'a [ShaderGroupDesc],

    /// TODO docs
    pub max_pipeline_ray_recursion_depth: u32,

    // const VkPipelineLibraryCreateInfoKHR*                pLibraryInfo;
    // const VkRayTracingPipelineInterfaceCreateInfoKHR*    pLibraryInterface;
    // const VkPipelineDynamicStateCreateInfo*              pDynamicState;
    /// TODO docs
    pub layout: &'a B::PipelineLayout,

    /// TODO docs
    pub parent: BasePipeline<'a, B::RayTracingPipeline>,
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingShaderGroupCreateInfoKHR.html
#[derive(Debug)]
pub struct ShaderGroupDesc {
    /// TODO docs
    pub ty: GroupType,

    /// TODO docs
    /// is the index of the ray generation, miss, or callable shader from `RayTracingPipelineDesc::stages` in the group if the shader group has type of `GroupType::General`, and VK_SHADER_UNUSED_KHR otherwise.
    pub general_shader: u32,

    /// TODO docs
    /// is the optional index of the closest hit shader from `RayTracingPipelineDesc::stages` in the group if the shader group has type of `GroupType::TrianglesHitGroup` or `GroupType::ProceduralHitGroup`, and VK_SHADER_UNUSED_KHR otherwise.
    pub closest_hit_shader: u32,

    /// TODO docs
    /// is the optional index of the any-hit shader from `RayTracingPipelineDesc::stages` in the group if the shader group has type of `GroupType::TrianglesHitGroup` or `GroupType::ProceduralHitGroup`, and VK_SHADER_UNUSED_KHR otherwise.
    pub any_hit_shader: u32,

    /// TODO docs
    /// is the index of the intersection shader from `RayTracingPipelineDesc::stages` in the group if the shader group has type of `GroupType::ProceduralHitGroup`, and VK_SHADER_UNUSED_KHR otherwise.
    pub intersection_shader: u32,
    // TODO(capture-replay)
    // const void*                       pShaderGroupCaptureReplayHandle;
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingPipelineInterfaceCreateInfoKHR.html
#[derive(Debug)]
pub struct PipelineInterfaceDesc {
    max_pipeline_ray_payload_size: u32,
    max_pipeline_ray_hit_attribute_size: u32,
}
