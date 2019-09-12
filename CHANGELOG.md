# Change Log

### backend-dx12-0.3.3 (12-09-2019)
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
  - fix depth clip mode support, updates spirv-cross

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
