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

/// A description of the data needed to construct a ray tracing pipeline.
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingPipelineCreateInfoKHR.html
#[derive(Debug)]
pub struct RayTracingPipelineDesc<'a, B: Backend> {
    /// Pipeline label
    pub label: Option<&'a str>,

    /// TODO docs
    pub flags: PipelineCreationFlags,

    /// TODO docs
    // todo shaderstagecreatedesc instead
    pub stages: &'a [ShaderStageDesc<'a, B>],

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

impl<'a, B: Backend> RayTracingPipelineDesc<'a, B> {
    /// Create a new empty PSO descriptor.
    pub fn new(
        stages: &'a [ShaderStageDesc<'a, B>],
        groups: &'a [ShaderGroupDesc],
        max_pipeline_ray_recursion_depth: u32,
        layout: &'a B::PipelineLayout,
    ) -> Self {
        Self {
            label: None,
            flags: PipelineCreationFlags::empty(),
            stages,
            groups,
            max_pipeline_ray_recursion_depth,
            layout,
            parent: BasePipeline::None,
        }
    }
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkPipelineShaderStageCreateInfo.html
#[derive(Debug)]
pub struct ShaderStageDesc<'a, B: Backend> {
    /// TODO docs
    pub stage: ShaderStageFlags,
    /// TODO docs
    pub entry_point: EntryPoint<'a, B>,
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingShaderGroupCreateInfoKHR.html
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingShaderGroupTypeKHR.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderGroupDesc {
    /// Specifies a shader group with a single shader in it.
    General {
        /// The index into the ray generation, miss, or callable shader from [`RayTracingPipelineDesc::stages`].
        general_shader: u32,
    },
    /// Specifies a shader group that only hits triangles.
    TrianglesHitGroup {
        /// The optional index into the closest hit shader from [`RayTracingPipelineDesc::stages`].
        closest_hit_shader: Option<u32>,
        /// The optional index into the any hit shader from [`RayTracingPipelineDesc::stages`].
        any_hit_shader: Option<u32>,
    },
    /// Specifies a shader group that only intersects with custom geometry.
    ProceduralHitGroup {
        /// The optional index into the closest hit shader from [`RayTracingPipelineDesc::stages`].
        closest_hit_shader: Option<u32>,
        /// The optional index into the any hit shader from [`RayTracingPipelineDesc::stages`].
        any_hit_shader: Option<u32>,
        /// The index into the intersection shader from [`RayTracingPipelineDesc::stages`].
        intersection_shader: u32,
    },
}

/// TODO docs
// https://www.khronos.org/registry/vulkan/specs/1.2-extensions/man/html/VkRayTracingPipelineInterfaceCreateInfoKHR.html
#[derive(Debug)]
pub struct PipelineInterfaceDesc {
    max_pipeline_ray_payload_size: u32,
    max_pipeline_ray_hit_attribute_size: u32,
}
