//! Ray tracing pipeline descriptor.

use crate::{
    buffer::{Offset, Stride},
    pso::{PipelineCreationFlags, ShaderStageFlags},
    Backend,
};

use super::EntryPoint;

/// TODO docs
pub const SHADER_UNUSED: u32 = !0;

/// TODO docs
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
#[derive(Debug)]
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
#[derive(Debug)]
pub enum VkRayTracingShaderGroupTypeKHR {
    /// TODO docs
    General,
    /// TODO docs
    TrianglesHitGroup,
    /// TODO docs
    ProceduralHitGroup,
}

/// A description of the data needed to construct a ray tracing pipeline.
#[derive(Debug)]
pub struct RayTracingPipelineDesc<'a, B: Backend> {
    /// TODO docs
    pub flags: PipelineCreationFlags,

    /// TODO docs
    pub shader_stages: &'a [EntryPoint<'a, B>],

    /// TODO docs
    pub shader_groups: &'a [ShaderGroupDesc],

    /// TODO docs
    pub max_pipeline_ray_recursion_depth: u32,

    // const VkPipelineLibraryCreateInfoKHR*                pLibraryInfo;
    // const VkRayTracingPipelineInterfaceCreateInfoKHR*    pLibraryInterface;
    // const VkPipelineDynamicStateCreateInfo*              pDynamicState;
    /// TODO docs    
    pub layout: &'a B::PipelineLayout,
    // VkPipeline                                           basePipelineHandle;
    // int32_t                                              basePipelineIndex;
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkPipelineShaderStageCreateInfo.html
#[derive(Debug)]
pub struct ShaderGroupDesc {
    // VkPipelineShaderStageCreateFlags    flags;
    /// TODO docs
    stage: ShaderStageFlags,
    // VkShaderModule                      module;
    // const char*                         pName;
    // const VkSpecializationInfo*         pSpecializationInfo;
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingPipelineInterfaceCreateInfoKHR.html
#[derive(Debug)]
pub struct PipelineInterfaceDesc {
    max_pipeline_ray_payload_size: u32,
    max_pipeline_ray_hit_attribute_size: u32,
}
