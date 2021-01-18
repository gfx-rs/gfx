# Change Log

### hal-unreleased
  - error improvements:
    - use `thiserror` for errors
    - variants and a few names are refactored
  - API external synchronization constraints now match Vulkan, `&mut` changes affected the following parameters:
    - event in `Device::set_event` and `Device::reset_event`
    - fence in `Device::reset_fences` and `Queue::submit`
    - destination sets in `write_descriptor_sets` and `copy_descriptor_sets`
    - memory in `map_memory` and `unmap_memory`
    - queue in `Queue::wait_idle`
    - semaphore in `Queue::present`
  - Borrowing is removed from the API, and `ExactSizeIterator` bounds are inserted where they were missing
  - `ImageFeature` improvements:
    - new `STORAGE_READ_WRITE` bit, indicating that the storage can be read and written within the same draw/dispatch call
    - new `TRANSFER_SRC` and `TRANSFER_DST` bits, following `VK_KHR_maintenance1`
    - new `SAMPLED_MINMAX` bit, following `VK_EXT_sampling_minmax`
  - Framebuffers become image-less, following `VK_KHR_imageless_framebuffer`
  - the old swapchain model is removed, and the new one is updated to match the backends even better
  - debug names are supported for all objectr
  - other API changes:
    - `bind_index_buffer` now doesn't need a separate structure
    - plural versions of `reset_fence` and `create_xx_pipeline` are removed
    - swapchain images can be used for transfer operations
    - separate feature for comparison mutable samplers
    - pipeline descriptor vectors are replaced with slices
    - features for non-normalized mutable samplers
    - `Capabilities` structure with supported dynamic state flags
  - OpenGL backend improvements:
    - finally has the API fully matching gfx-hal
    - now only uses OpenGL ES on Linux/Android/Web targets
    - binding model has been completely rewritten
    - various number of fixed in rendering, memory mapping, and other areas


### backend-dx12-unreleased
  - fix SPIR-V entry point selection

### backend-vulkan-0.6.5 (15-10-2020)
  - support different types of descriptors in a single `DescriptorSetWrite`

### backend-dx12-0.6.3 backend-dx11-0.6.1 backend-metal-0.6.2 auxil-0.5.1 (31-08-2020)
  - update spirv_cross to 0.21:
    - force zero initialization in all generated shaders
    - force the use of native arrays for MSL

### backend-dx12-0.6.7 (12-10-2020)
  - get proper support for compressed textures

### backend-dx12-0.6.6 (05-10-2020)
  - allow color blend factors to be used on alpha channel

### backend-dx12-0.6.5 (04-10-2020)
  - implement command buffer markers
  - debug names for render passes and descriptor sets

### backend-vulkan-0.6.3 (30-09-2020)
  - enable VK_KHR_maintenance3 when VK_EXT_descriptor_indexing is enabled

### backend-dx12-0.6.4 backend-vulkan-0.6.2 backend-metal-0.6.3 (23-09-2020)
  - fix descriptor indexing features

### backend-dx11-0.6.4 (07-09-2020)
  - fix memory flush ranges
  - support presentation modes

### backend-dx11-0.6.3 (04-09-2020)
  - fix cpu-visible mapping
  - fix UAV reset count

### backend-dx11-0.6.2 (02-09-2020)
  - fix bindings filter by shader stages
  - implement copies from buffers into R8, RG8, and RGBA8 textures
  - fix read-only storage buffer support
  - fix race condition in internal shader operations

### auxil-0.6.0 (02-09-2020)
  - update to newer version of spirv_cross to be consistent with backends

### backend-dx12-0.6.3 (02-09-2020)
  - fix root signature indexing
  - force zero initialization for shader variables

### backend-metal-0.6.2 (02-09-2020)
  - enable compatibility with iOS emulator
  - force zero initialization for shader variables
  - force the use of native arrays for MSL

### backend-dx11-0.6.1 (02-09-2020)
  - force zero initialization for shader variables

### backend-metal-0.6.1 (23-08-2020)
  - fix layer checks in `clear_image`

### backend-dx12-0.6.2 (19-08-2020)
  - enable multisampling and object labels

### backend-dx12-0.6.1 (18-08-2020)
  - fix descriptor binding

### backend-vulkan-0.6.1 (17-08-2020)
  - fix Android build

## hal-0.6.0 (15-08-2020)
  - API changes:
    - the old Vulkan-ish swapchain model is completely removed
    - `pso::Stage` enum is removed from the API into `gfx-auxil::ShaderStage`
    - `SubresourceRange` allows unbound array layers and mipmap levels
    - new `PrimitiveAssemblerDesc` enum
    - `DescriptorPool::free_sets` is renamed to just `free`
  - Features:
    - object labels for pipelines and their layouts
    - draw with indirect counts
    - mesh shaders (Vulkan with NV extension only, for now)

### backend-dx12-0.5.10 (16-08-2020)
  - fix binding of dynamic uniform buffers

### backend-dx12-0.5.9 (14-08-2020)
  - fix creation of depth-stencil views
  - fix command allocator reset validation errors
  - fix the crash on `unconfigure_swapchain`

### backend-dx11-0.5.2 (29-07-2020)
  - update libloading to 0.6

### backend-vulkan-0.5.11 (22-07-2020)
  - switch from `core-graphics` to `core-graphics-types`.

### backend-metal-0.5.6 (21-07-2020)
  - update metal to 0.20
  - switch from `cocoa` to `cocoa-foundation`.
  - remove core-graphics dependency

### backend-metal-0.5.5 (20-07-2020)
  - update cocoa to 0.22 and metal to 0.19.

### backend-vulkan-0.5.10 (10-07-2020)
  - skip unknown memory types

### backend-empty-0.5.2 (06-07-2020)
  - mock descriptor set creation functions

### backend-empty-0.5.1 (30-06-2020)
  - start turning the empty backend into a mock instead of always panicking
  - mock memory creation and buffer and image creation functions

### hal-0.5.3 backend-dx12-0.5.8 backend-vulkan-0.5.9 (27-06-2020)
  - add `DRAW_INDIRECT_COUNT` feature and enable on supported backends

### hal-0.5.2 backend-dx12-0.5.7 backend-metal-0.5.4 backend-vulkan-0.5.8 (12-06-2020)
  - add descriptor indexing features and enable on supported backends

### hal-0.5.1 backend-dx12-0.5.6 backend-metal-0.5.3 backend-vulkan-0.5.7 (10-06-2020)
  - add `TEXTURE_DESCRIPTOR_ARRAY` feature and enable on supported backends

### backend-dx12-0.5.5 (01-06-2020)
  - implement descriptor pool destruction

### backend-dx12-0.5.4 (29-05-2020)
  - fix detection of integrated gpus
  - fix UB in `compile_shader`

### backend-dx11-0.5.1, backend-dx12-0.5.3, backend-gl-0.5.1, backend-metal-0.5.2 (05-05-2020)
  - update spirv_cross to 0.20

### backend-dx12-0.5.2 (05-04-2020)
  - fix offset calculation for root descriptors

### backend-dx12-0.5.1 (01-01-2020)
  - fix drop of physical devices
  - handle device lost during a fence wait
  - rework the way swapchain waits to acquire new frames

### backend-vulkan-0.5.6 (27-04-2020)
  - gracefully detect when the driver supports it but hardware does not

### backend-vulkan-0.5.3 (25-04-2020)
  - switch to `VK_LAYER_KHRONOS_validation`

### backend-vulkan-0.5.2 (01-04-2020)
  - fix support for `AMD_NEGATIVE_VIEWPORT_HEIGHT`

### backend-metal-0.5.1 (26-03-2020)
  - fix debug assertion for the index buffer range
  - fix `NDC_Y_FLIP` feature

### backend-vulkan-0.5.1 (26-03-2020)
  - fix debug color markers
  - fix detection of the `MirrorClamp` mode

## hal-0.5.0 (23-03-2020)
  - API changes:
    - allocating command buffers or descriptor sets no longer touches the heap
    - `DescriptorType` is now a rich enum
    - `RangeArg` trait is removed, instead the offsets are required, and the sizes are optional
    - Removed `Anisotropic` and `SubpassRef` enums in favor of options
  - Features:
    - debug markers
    - new `WrapMode::MirrorClamp`
    - Y-flipped NDC space
    - read-only storage descriptors

### backend-metal-0.4.3 (22-02-2020)
  - support version 2.2 of the Metal shading language

### backend-vulkan-0.4.2 (13-02-2020)
  - work around Intel swapchain bug when acquiring images

### backend-dx12-0.4.3 (07-01-2020)
  - fix a crash at initialization time

### backend-dx11-0.4.4, backend-dx11-0.4.5 (06-01-2020)
  - disable coherent memory for being broken
  - rewrite the binding model completely

### backend-metal-0.4.2 (18-11-2019)
  - fix missing iOS metallib
  - fix viewport/scissor after `clear_attachments` call

### hal-0.4.1 (04-11-2019)
  - `Error` implementations
  - fix `ShaderStageFlags::ALL`

### backend-dx12-0.4.1, backend-dx11-0.4.2 (01-11-2019)
  - switch to explicit linking of "d3d12.dll", "d3d11.dll" and "dxgi.dll"

### backend-dx12-0.4.1 (01-11-2019)
  - switch to explicit linking of "d3d12.dll" and "dxgi.dll"

## hal-0.4.0 (23-10-2019)
  - all strongly typed HAL wrappers are removed
  - all use of `failure` is removed
  - alternative swapchain model built into `Surface`
  - `Instance` trait is assocated by `Backend`, now includes surface creation and destruction
  - `Surface` capabiltities queried are refactored, `PresentMode` is turned into bitflags
  - `Primitive` enum is refactored and moved to `pso` module
  - `SamplerInfo` struct is refactored and renamed to `SamplerDesc`
  - debug labels for objects

### backend-dx12-0.3.4 (13-09-2019)
  - improve external render pass barriers

### backend-metal-0.3.3 (05-09-2019)
  - fix immutable samplers in combined image-samplers

### backend-vulkan-0.3.3 (03-09-2019)
  - fix iOS build

### backend-vulkan-0.3.2, backend-dx12-0.3.2 (30-08-2019)
  - add `Instance::try_create` methods

### backend-metal-0.3.1 (21-08-2019)
  - fix memory leaks in render pass and labels creation

## hal-0.3.0 (08-08-2019)
  - graphics pipeline state refactor
  - no `winit` feature by default
  - events support
  - more device limits are exposed
  - Vulkan: fixed swapchain ranges, stencil dynamic states
  - DX12: "readonly" storage support
  - Metal: argument buffer support, real immutable samplers
  - GL: compute shaders, new memory model, WebGL support, lots of other goodies

### backend-dx12-0.2.4 (02-08-2019)
  - optimize shader visibility of descriptors

### backend-dx12-0.2.3, backend-metal-0.2.4 (01-08-2019)
  - fix exposed MSAA capabilities and resolves

### backend-dx12-0.2.2 (29-07-2019)
  - fix image view creation panics

### backend-backend-metal-0.2.3 (10-07-2019)
  - fixed depth clip mode support, updates spirv-cross

### backend-dx11-0.2.1, backend-dx12-0.2.1, backend-metal-0.2.2, backend-empty-0.2.1 (28-06-2019)
  - `Debug` implementations for `Instance`

### backend-vulkan-0.2.2 (14-06-2019)
  - allow building on macOS for Vulkan Portability

### backend-metal-0.2.1 (14-06-2019)
  - fixed memory leaks in render pass descriptors and function strings

### hal-0.2.1 (10-06-2019)
  - `Debug` implementations

### backend-vulkan-0.2.1 (23-05-2019)
  - fix `VK_EXT_debug_utils` check at startup

## hal-0.2.0 (10-05-2019)
  - pipeline cache support
  - rich presentation errors
  - nicer specialization constants
  - `Debug` implementations
  - consistent format names
  - more limits
  - surface alpha composition properties
  - descriptor pool create flags
  - removal of `FrameSync`

### backend-dx11-0.1.1 (05-03-2019)
  - fixed buffer bind flags
  - synchronization of disjoint CB across copy operations
  - depth texture views

### backend-dx12-0.1.2 (04-03-2019)
  - typeless formats for textures
  - fixed vertex buffer binding
  - fixed non-array views of array textures

### backend-metal-0.1.1 (21-02-2019)
  - secondary command buffers
  - multiple iOS fixes
  - fixed surface dimensions

### backend-dx12-0.1.1 (04-02-2019)
  - `get_fence_status`

### backend-empty-0.1.0 (04-02-2019)
  - dummy surface creation

## hal-0.1.0 (27-12-2018)
  - `gfx-hal`: graphics hardware abstraction layer
  - `gfx-backend-*`: Vulkan, D3D12, D3D11, Metal, and GL
  - `range-alloc`: helper struct to manage ranges
  - unsafe qualifiers on all the API methods
  - non-clonable command buffers and resources
