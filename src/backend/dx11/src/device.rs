use hal;
use hal::queue::QueueFamilyId;
use hal::range::RangeArg;
use hal::{buffer, device, error, format, image, mapping, memory, pass, pool, pso, query, window};

use winapi::Interface;
use winapi::shared::dxgi::{IDXGISwapChain, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD};
use winapi::shared::minwindef::{TRUE};
use winapi::shared::{dxgiformat, dxgitype, winerror};
use winapi::um::{d3d11, d3dcommon};

use wio::com::ComPtr;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::mem;
use std::ops::Range;
use std::ptr;

use {
    Backend, Buffer, BufferView, CommandPool, ComputePipeline, DescriptorPool, DescriptorSetLayout,
    Fence, Framebuffer, GraphicsPipeline, Image, ImageView, InternalBuffer, InternalImage, Memory,
    PipelineLayout, QueryPool, RenderPass, Sampler, Semaphore, ShaderModule, Surface, Swapchain,
    UnboundBuffer, UnboundImage, ViewInfo, PipelineBinding, Descriptor
};

use {conv, internal, shader};

pub struct Device {
    raw: ComPtr<d3d11::ID3D11Device>,
    pub(crate) context: ComPtr<d3d11::ID3D11DeviceContext>,
    memory_properties: hal::MemoryProperties,
    pub(crate) internal: internal::Internal
}

unsafe impl Send for Device { }
unsafe impl Sync for Device { }

impl Device {
    pub fn as_raw(&self) -> *mut d3d11::ID3D11Device {
        self.raw.as_raw()
    }

    pub fn new(device: ComPtr<d3d11::ID3D11Device>, context: ComPtr<d3d11::ID3D11DeviceContext>, memory_properties: hal::MemoryProperties) -> Self {
        Device {
            raw: device.clone(),
            context,
            memory_properties,
            internal: internal::Internal::new(&device)
        }
    }

    fn create_rasterizer_state(&self, rasterizer_desc: &pso::Rasterizer) -> Result<ComPtr<d3d11::ID3D11RasterizerState>, pso::CreationError> {
        let mut rasterizer = ptr::null_mut();
        let desc = conv::map_rasterizer_desc(rasterizer_desc);

        let hr = unsafe {
            self.raw.CreateRasterizerState(
                &desc,
                &mut rasterizer as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(rasterizer) })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_blend_state(&self, blend_desc: &pso::BlendDesc) -> Result<ComPtr<d3d11::ID3D11BlendState>, pso::CreationError> {
        let mut blend = ptr::null_mut();
        let desc = conv::map_blend_desc(blend_desc);

        let hr = unsafe {
            self.raw.CreateBlendState(
                &desc,
                &mut blend as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(blend) })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_depth_stencil_state(&self, depth_desc: &pso::DepthStencilDesc) -> Result<(ComPtr<d3d11::ID3D11DepthStencilState>, pso::State<pso::StencilValue>), pso::CreationError> {
        let mut depth = ptr::null_mut();
        let (desc, stencil_ref) = conv::map_depth_stencil_desc(depth_desc);

        let hr = unsafe {
            self.raw.CreateDepthStencilState(
                &desc,
                &mut depth as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok((unsafe { ComPtr::from_raw(depth) }, stencil_ref))
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_input_layout(&self, vs: ComPtr<d3dcommon::ID3DBlob>, vertex_buffers: &[pso::VertexBufferDesc], attributes: &[pso::AttributeDesc], input_assembler: &pso::InputAssemblerDesc) -> Result<([u32; d3d11::D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT as usize], d3d11::D3D11_PRIMITIVE_TOPOLOGY, ComPtr<d3d11::ID3D11InputLayout>), pso::CreationError> {
        let mut layout = ptr::null_mut();

        let mut vertex_strides = [0u32; d3d11::D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT as usize];
        for buffer in vertex_buffers {
            vertex_strides[buffer.binding as usize] = buffer.stride;
        }

        let input_elements = attributes
            .iter()
            .filter_map(|attrib| {
                let buffer_desc = match vertex_buffers
                    .iter().find(|buffer_desc| buffer_desc.binding == attrib.binding)
                {
                    Some(buffer_desc) => buffer_desc,
                    None => {
                        // TODO:
                        // error!("Couldn't find associated vertex buffer description {:?}", attrib.binding);
                        return Some(Err(pso::CreationError::Other));
                    }
                };

                let slot_class = match buffer_desc.rate {
                    0 => d3d11::D3D11_INPUT_PER_VERTEX_DATA,
                    _ => d3d11::D3D11_INPUT_PER_INSTANCE_DATA,
                };
                let format = attrib.element.format;

                Some(Ok(d3d11::D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: "TEXCOORD\0".as_ptr() as *const _, // Semantic name used by SPIRV-Cross
                    SemanticIndex: attrib.location,
                    Format: match conv::map_format(format) {
                        Some(fm) => fm,
                        None => {
                            // TODO:
                            // error!("Unable to find DXGI format for {:?}", format);
                            return Some(Err(pso::CreationError::Other));
                        }
                    },
                    InputSlot: attrib.binding as _,
                    AlignedByteOffset: attrib.element.offset,
                    InputSlotClass: slot_class,
                    InstanceDataStepRate: buffer_desc.rate as _,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let hr = unsafe {
            self.raw.CreateInputLayout(
                input_elements.as_ptr(),
                input_elements.len() as _,
                vs.GetBufferPointer(),
                vs.GetBufferSize(),
                &mut layout as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            let topology = conv::map_topology(input_assembler.primitive);

            Ok((vertex_strides, topology, unsafe { ComPtr::from_raw(layout) }))
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_vertex_shader(&self, blob: ComPtr<d3dcommon::ID3DBlob>) -> Result<ComPtr<d3d11::ID3D11VertexShader>, pso::CreationError> {
        let mut vs = ptr::null_mut();

        let hr = unsafe {
            self.raw.CreateVertexShader(
                blob.GetBufferPointer(),
                blob.GetBufferSize(),
                ptr::null_mut(),
                &mut vs as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(vs) })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_pixel_shader(&self, blob: ComPtr<d3dcommon::ID3DBlob>) -> Result<ComPtr<d3d11::ID3D11PixelShader>, pso::CreationError> {
        let mut ps = ptr::null_mut();

        let hr = unsafe {
            self.raw.CreatePixelShader(
                blob.GetBufferPointer(),
                blob.GetBufferSize(),
                ptr::null_mut(),
                &mut ps as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(ps) })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    // TODO: fix return type..
    fn extract_entry_point(
        stage: pso::Stage,
        source: &pso::EntryPoint<Backend>,
        layout: &PipelineLayout,
    ) -> Result<Option<ComPtr<d3dcommon::ID3DBlob>>, device::ShaderError> {
        // TODO: entrypoint stuff
        match *source.module {
            ShaderModule::Dxbc(ref _shader) => {
                unimplemented!()

                // Ok(Some(shader))
            }
            ShaderModule::Spirv(ref raw_data) => {
                Ok(shader::compile_spirv_entrypoint(raw_data, stage, source, layout)?)
            }
        }
    }

    fn view_image_as_shader_resource(&self, info: &ViewInfo) -> Result<ComPtr<d3d11::ID3D11ShaderResourceView>, image::ViewError> {
        let mut desc: d3d11::D3D11_SHADER_RESOURCE_VIEW_DESC = unsafe { mem::zeroed() };
        desc.Format = info.format;

        #[allow(non_snake_case)]
        let MostDetailedMip = info.range.levels.start as _;
        #[allow(non_snake_case)]
        let MipLevels = (info.range.levels.end - info.range.levels.start) as _;

        match info.view_kind {
            image::ViewKind::D2 => {
                desc.ViewDimension = d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d11::D3D11_TEX2D_SRV {
                    MostDetailedMip,
                    MipLevels,
                }
            },
            _ => unimplemented!()
        }

        let mut srv = ptr::null_mut();
        let hr = unsafe {
            self.raw.CreateShaderResourceView(
                info.resource,
                &desc,
                &mut srv as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(srv) })
        } else {
            Err(image::ViewError::Unsupported)
        }
    }

    fn view_image_as_unordered_access_view(&self, info: &ViewInfo) -> Result<ComPtr<d3d11::ID3D11UnorderedAccessView>, image::ViewError> {
        let mut desc: d3d11::D3D11_UNORDERED_ACCESS_VIEW_DESC = unsafe { mem::zeroed() };
        desc.Format = info.format;

        match info.view_kind {
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d11::D3D11_UAV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d11::D3D11_TEX2D_UAV {
                    MipSlice: info.range.levels.start as _,
                }
            },
            _ => unimplemented!()
        }

        let mut uav = ptr::null_mut();
        let hr = unsafe {
            self.raw.CreateUnorderedAccessView(
                info.resource,
                &desc,
                &mut uav as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(uav) })
        } else {
            Err(image::ViewError::Unsupported)
        }
    }

    fn view_image_as_render_target(&self, info: &ViewInfo) -> Result<ComPtr<d3d11::ID3D11RenderTargetView>, image::ViewError> {
        let mut desc: d3d11::D3D11_RENDER_TARGET_VIEW_DESC = unsafe { mem::zeroed() };
        desc.Format = info.format;

        match info.view_kind {
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d11::D3D11_RTV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d11::D3D11_TEX2D_RTV {
                    MipSlice: 0,
                }
            },
            _ => unimplemented!()
        }

        let mut rtv = ptr::null_mut();
        let hr = unsafe {
            self.raw.CreateRenderTargetView(
                info.resource,
                &desc,
                &mut rtv as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(rtv) })
        } else {
            Err(image::ViewError::Unsupported)
        }
    }

    fn view_image_as_depth_stencil(&self, info: &ViewInfo) -> Result<ComPtr<d3d11::ID3D11DepthStencilView>, image::ViewError> {
        let mut desc: d3d11::D3D11_DEPTH_STENCIL_VIEW_DESC = unsafe { mem::zeroed() };
        desc.Format = info.format;

        match info.view_kind {
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d11::D3D11_DSV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d11::D3D11_TEX2D_DSV {
                    MipSlice: 0,
                }
            },
            _ => unimplemented!()
        }

        let mut dsv = ptr::null_mut();
        let hr = unsafe {
            self.raw.CreateDepthStencilView(
                info.resource,
                &desc,

                &mut dsv as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(dsv) })
        } else {
            Err(image::ViewError::Unsupported)
        }
    }
}

impl hal::Device<Backend> for Device {
    fn allocate_memory(
        &self,
        mem_type: hal::MemoryTypeId,
        size: u64,
    ) -> Result<Memory, device::OutOfMemory> {
        let working_buffer_size = 1 << 15;
        let working_buffer = if mem_type.0 == 1 {
            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: working_buffer_size,
                Usage: d3d11::D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: d3d11::D3D11_CPU_ACCESS_READ | d3d11::D3D11_CPU_ACCESS_WRITE,
                MiscFlags:0,
                StructureByteStride: 0,

            };
            let mut working_buffer = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateBuffer(
                    &desc,
                    ptr::null_mut(),
                    &mut working_buffer as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                return Err(device::OutOfMemory);
            }

            Some(unsafe { ComPtr::from_raw(working_buffer) })
        } else {
            None
        };

        Ok(Memory {
            properties: self.memory_properties.memory_types[mem_type.0].properties,
            size,
            mapped_ptr: RefCell::new(None),
            host_visible: Some(RefCell::new(Vec::with_capacity(size as usize))),
            working_buffer,
            working_buffer_size: working_buffer_size as u64,
            local_buffers: RefCell::new(Vec::new()),
            local_images: RefCell::new(Vec::new()),
        })
    }

    fn create_command_pool(
        &self, _family: QueueFamilyId, _create_flags: pool::CommandPoolCreateFlags
    ) -> CommandPool {
        // TODO:
        CommandPool {
            device: self.raw.clone(),
            internal: self.internal.clone(),
        }
    }

    fn destroy_command_pool(&self, _pool: CommandPool) {
        // TODO:
        // unimplemented!()
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        _attachments: IA,
        _subpasses: IS,
        _dependencies: ID,
    ) -> RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        // TODO: renderpass

        RenderPass
    }

    fn create_pipeline_layout<IS, IR>(
        &self,
        set_layouts: IS,
        _push_constant_ranges: IR,
    ) -> PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        let mut set_bindings = Vec::new();

        for layout in set_layouts {
            let layout = layout.borrow();

            let bindings = &layout.bindings;

            let stages = [
                pso::ShaderStageFlags::VERTEX,
                pso::ShaderStageFlags::HULL,
                pso::ShaderStageFlags::DOMAIN,
                pso::ShaderStageFlags::GEOMETRY,
                pso::ShaderStageFlags::FRAGMENT,
                pso::ShaderStageFlags::COMPUTE,
            ];

            let mut optimized_bindings = Vec::new();

            // for every shader stage we get a range of descriptor handles that can be bound with
            // PS/VS/CSSetXX()
            for &stage in &stages {
                let mut current_type = None;
                let mut current_range = None;
                // track the starting offset of the handles
                let mut start_offset = 0;
                // and where our current tail of the range is
                let mut current_offset = 0;

                for binding in bindings {
                    match (current_type, current_range.clone()) {
                        (None, None) => {
                            if binding.stage.contains(stage) {
                                current_type = Some(binding.ty);
                                current_range = Some(binding.binding_range.clone());
                                start_offset = binding.handle_offset;
                            }
                        }
                        (Some(ty), Some(ref mut range)) => {
                            // if we encounter another type or the binding/handle
                            // range is broken, push our current descriptor range
                            // and begin a new one.
                            if ty != binding.ty ||
                               (range.end) != binding.binding_range.start ||
                               (current_offset + 1) != binding.handle_offset ||
                               stage != binding.stage
                            {
                                optimized_bindings.push(PipelineBinding {
                                    stage,
                                    ty,
                                    binding_range: range.clone(),
                                    handle_offset: start_offset
                                });
                            
                                if binding.stage.contains(stage) {
                                    current_type = Some(binding.ty);
                                    current_range = Some(binding.binding_range.clone());

                                    start_offset = binding.handle_offset;
                                    current_offset = binding.handle_offset;
                                } else {
                                    current_type = None;
                                    current_range = None;
                                }
                            } else {
                                range.end += 1;
                                current_offset += 1;
                            }
                        }
                        // either both Something, or both Nonething
                        _ => unreachable!()
                    }
                }

                // catch trailing descriptors
                if let (Some(ty), Some(range)) = (current_type, &mut current_range) {
                    optimized_bindings.push(PipelineBinding {
                        stage,
                        ty,
                        binding_range: range.clone(),
                        handle_offset: current_offset
                    });
                }
            }

            set_bindings.push(optimized_bindings);
        }

        PipelineLayout {
            set_bindings
        }
    }

    fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, Backend>,
    ) -> Result<GraphicsPipeline, pso::CreationError> {
        let build_shader =
            |stage: pso::Stage, source: Option<&pso::EntryPoint<'a, Backend>>| {
                let source = match source {
                    Some(src) => src,
                    None => return Ok(None),
                };

                Self::extract_entry_point(stage, source, desc.layout)
                    .map_err(|err| pso::CreationError::Shader(err))
            };

        let vs = build_shader(pso::Stage::Vertex, Some(&desc.shaders.vertex))?.unwrap();
        let ps = build_shader(pso::Stage::Fragment, desc.shaders.fragment.as_ref())?;
        // TODO:
        /*let gs = build_shader(pso::Stage::Geometry, desc.shaders.geometry.as_ref())?;
        let ds = build_shader(pso::Stage::Domain, desc.shaders.domain.as_ref())?;
        let hs = build_shader(pso::Stage::Hull, desc.shaders.hull.as_ref())?;*/

        let (strides, topology, input_layout) = self.create_input_layout(vs.clone(), &desc.vertex_buffers, &desc.attributes, &desc.input_assembler)?;
        let rasterizer_state = self.create_rasterizer_state(&desc.rasterizer)?;
        let blend_state = self.create_blend_state(&desc.blender)?;
        let depth_stencil_state = Some(self.create_depth_stencil_state(&desc.depth_stencil)?);

        let vs = self.create_vertex_shader(vs)?;
        let ps = if let Some(blob) = ps {
            Some(self.create_pixel_shader(blob)?)
        } else {
            None
        };

        Ok(GraphicsPipeline {
            vs,
            ps,
            topology,
            input_layout,
            rasterizer_state,
            blend_state,
            depth_stencil_state,
            baked_states: desc.baked_states.clone(),
            strides,
        })
    }

    fn create_compute_pipeline<'a>(
        &self,
        _desc: &pso::ComputePipelineDesc<'a, Backend>,
    ) -> Result<ComputePipeline, pso::CreationError> {
        unimplemented!()
    }

    fn create_framebuffer<I>(
        &self,
        _renderpass: &RenderPass,
        attachments: I,
        extent: image::Extent,
    ) -> Result<Framebuffer, device::FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<ImageView>
    {
        Ok(Framebuffer {
            attachments: attachments.into_iter().map(|att| att.borrow().clone()).collect(),
            layers: extent.depth as _,
        })
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<ShaderModule, device::ShaderError> {
        Ok(ShaderModule::Spirv(raw_data.into()))
    }

    fn create_buffer(
        &self,
        size: u64,
        usage: buffer::Usage,
    ) -> Result<UnboundBuffer, buffer::CreationError> {
        use buffer::Usage;

        let mut bind = 0;

        if usage.contains(Usage::UNIFORM) { bind |= d3d11::D3D11_BIND_CONSTANT_BUFFER; }
        if usage.contains(Usage::VERTEX) { bind |= d3d11::D3D11_BIND_VERTEX_BUFFER; }
        if usage.contains(Usage::INDEX) { bind |= d3d11::D3D11_BIND_INDEX_BUFFER; }

        // TODO: >=11.1
        if usage.contains(Usage::UNIFORM_TEXEL) ||
           usage.contains(Usage::STORAGE_TEXEL) ||
           usage.contains(Usage::TRANSFER_SRC) { bind |= d3d11::D3D11_BIND_SHADER_RESOURCE; }

        // TODO: how to do buffer copies
        if usage.contains(Usage::TRANSFER_DST) { bind |= d3d11::D3D11_BIND_UNORDERED_ACCESS; }

        Ok(UnboundBuffer {
            usage,
            bind,
            size,
            requirements: memory::Requirements {
                size,
                alignment: 1,
                type_mask: 0x7,
            }
        })
    }

    fn get_buffer_requirements(&self, buffer: &UnboundBuffer) -> memory::Requirements {
        buffer.requirements
    }

    fn bind_buffer_memory(
        &self,
        memory: &Memory,
        offset: u64,
        unbound_buffer: UnboundBuffer,
    ) -> Result<Buffer, device::BindError> {
        use memory::Properties;

        debug!("usage={:?}, props={:b}", unbound_buffer.usage, memory.properties);

        #[allow(non_snake_case)]
        let MiscFlags = if unbound_buffer.usage.contains(buffer::Usage::TRANSFER_SRC) {
            d3d11::D3D11_RESOURCE_MISC_BUFFER_STRUCTURED
        } else {
            0
        };

        let raw = if memory.properties == Properties::DEVICE_LOCAL {
            // device local memory
            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: unbound_buffer.size as _,
                Usage: d3d11::D3D11_USAGE_DEFAULT,
                BindFlags: unbound_buffer.bind,
                CPUAccessFlags: 0,
                MiscFlags,
                StructureByteStride: if unbound_buffer.usage.contains(buffer::Usage::TRANSFER_SRC) { 4 } else { 0 },
            };

            let mut buffer: *mut d3d11::ID3D11Buffer = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateBuffer(
                    &desc,
                    ptr::null_mut(),
                    &mut buffer as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                return Err(device::BindError::WrongMemory);
            }

            unsafe { ComPtr::from_raw(buffer) }
        } else if memory.properties == (Properties::CPU_VISIBLE) {
            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: unbound_buffer.size as _,
                // TODO: dynamic?
                Usage: d3d11::D3D11_USAGE_DEFAULT,
                BindFlags: unbound_buffer.bind,
                CPUAccessFlags: 0,
                MiscFlags,
                StructureByteStride: if unbound_buffer.usage.contains(buffer::Usage::TRANSFER_SRC) { 4 } else { 0 },
            };

            let mut buffer: *mut d3d11::ID3D11Buffer = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateBuffer(
                    &desc,
                    ptr::null_mut(),
                    &mut buffer as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                return Err(device::BindError::WrongMemory);
            }

            unsafe { ComPtr::from_raw(buffer) }
        } else {
            unimplemented!()
        };

        let srv = if unbound_buffer.usage.contains(buffer::Usage::TRANSFER_SRC) {
            let mut desc = unsafe { mem::zeroed::<d3d11::D3D11_SHADER_RESOURCE_VIEW_DESC>() };
            desc.Format = dxgiformat::DXGI_FORMAT_UNKNOWN;
            desc.ViewDimension = d3dcommon::D3D11_SRV_DIMENSION_BUFFER;
            unsafe {
                let mut buffer_srv = desc.u.Buffer_mut();
                *buffer_srv.u1.FirstElement_mut() = 0;
                *buffer_srv.u2.NumElements_mut() = unbound_buffer.size as u32 / 4;
            };

            let mut srv = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateShaderResourceView(
                    raw.as_raw() as *mut _,
                    &desc,
                    &mut srv as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                error!("CreateShaderResourceView failed: 0x{:x}", hr);

                return Err(device::BindError::WrongMemory);
            }

            Some(srv)
        } else {
            None
        };

        let uav = if unbound_buffer.usage.contains(buffer::Usage::TRANSFER_DST) {
            let mut desc = unsafe { mem::zeroed::<d3d11::D3D11_UNORDERED_ACCESS_VIEW_DESC>() };
            desc.Format = dxgiformat::DXGI_FORMAT_R32_UINT;
            desc.ViewDimension = d3d11::D3D11_UAV_DIMENSION_BUFFER;
            unsafe {
                *desc.u.Buffer_mut() = d3d11::D3D11_BUFFER_UAV {
                    FirstElement: 0,
                    NumElements: unbound_buffer.size as u32 / 4,
                    Flags: 0
                };
            };

            let mut uav = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateUnorderedAccessView(
                    raw.as_raw() as *mut _,
                    &desc,
                    &mut uav as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                error!("CreateUnorderedAccessView failed: 0x{:x}", hr);

                return Err(device::BindError::WrongMemory);
            }

            Some(uav)
        } else {
            None
        };

        let buffer = InternalBuffer {
            raw: raw.into_raw(),
            srv,
            uav,
        };
        let range = offset..unbound_buffer.size;

        memory.bind_buffer(range, buffer.clone());

        Ok(Buffer {
            internal: buffer,
            size: unbound_buffer.size
        })
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        _buffer: &Buffer,
        _format: Option<format::Format>,
        _range: R,
    ) -> Result<BufferView, buffer::ViewCreationError> {
        unimplemented!()
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        flags: image::StorageFlags,
    ) -> Result<UnboundImage, image::CreationError> {
        use image::Usage;
        //
        // TODO: create desc

        let surface_desc = format.base_format().0.desc();
        let bytes_per_texel  = surface_desc.bits / 8;
        let ext = kind.extent();
        let size = (ext.width * ext.height * ext.depth) as u64 * bytes_per_texel as u64;

        let mut bind = 0;

        if usage.contains(Usage::TRANSFER_SRC) ||
           usage.contains(Usage::SAMPLED) ||
           usage.contains(Usage::STORAGE) { bind |= d3d11::D3D11_BIND_SHADER_RESOURCE; }

        if usage.contains(Usage::COLOR_ATTACHMENT) ||
           usage.contains(Usage::TRANSFER_DST) { bind |= d3d11::D3D11_BIND_RENDER_TARGET; }
        if usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) { bind |= d3d11::D3D11_BIND_DEPTH_STENCIL; }

        // TODO: how to do buffer copies
        if usage.contains(Usage::TRANSFER_DST) { bind |= d3d11::D3D11_BIND_UNORDERED_ACCESS; }

        debug!("{:b}", bind);

        Ok(UnboundImage {
            kind,
            mip_levels,
            format,
            tiling,
            usage,
            flags,
            bind,
            // TODO:
            requirements: memory::Requirements {
                size: size,
                alignment: 1,
                type_mask: 0x1,
            },
        })
    }

    fn get_image_requirements(&self, image: &UnboundImage) -> memory::Requirements {
        image.requirements
    }

    fn get_image_subresource_footprint(
        &self, _image: &Image, _sub: image::Subresource
    ) -> image::SubresourceFootprint {
        unimplemented!()
    }

    fn bind_image_memory(
        &self,
        memory: &Memory,
        _offset: u64,
        image: UnboundImage,
    ) -> Result<Image, device::BindError> {
        use memory::Properties;
        use image::Usage;

        let base_format = image.format.base_format();
        let format_desc = base_format.0.desc();
        let bytes_per_block = (format_desc.bits / 8) as _;
        let block_dim = format_desc.dim;

        let (bind, usage, cpu) = if memory.properties == Properties::DEVICE_LOCAL {
            (image.bind, d3d11::D3D11_USAGE_DEFAULT, 0)
        } else if memory.properties == (Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::CPU_CACHED) {
            (image.bind, d3d11::D3D11_USAGE_DYNAMIC, d3d11::D3D11_CPU_ACCESS_WRITE)
        } else if memory.properties == (Properties::CPU_VISIBLE | Properties::CPU_CACHED) {
            (0, d3d11::D3D11_USAGE_STAGING, d3d11::D3D11_CPU_ACCESS_READ | d3d11::D3D11_CPU_ACCESS_WRITE)
        } else {
            unimplemented!()
        };

        let dxgi_format = conv::map_format(image.format).unwrap();
        let (typeless_format, typed_raw_format) = conv::typeless_format(dxgi_format).unwrap();

        let (resource, levels) = match image.kind {
            image::Kind::D2(width, height, layers, _) => {

                debug!("{:b}", bind);
                let desc = d3d11::D3D11_TEXTURE2D_DESC {
                    Width: width,
                    Height: height,
                    MipLevels: image.mip_levels as _,
                    ArraySize: layers as _,
                    Format: typeless_format,
                    SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0
                    },
                    Usage: usage,
                    BindFlags: bind,
                    CPUAccessFlags: cpu,
                    MiscFlags: 0
                };

                let mut resource = ptr::null_mut();
                let hr = unsafe {
                    self.raw.CreateTexture2D(
                        &desc,
                        ptr::null_mut(),
                        &mut resource as *mut *mut _ as *mut *mut _
                    )
                };

                if !winerror::SUCCEEDED(hr) {
                    error!("CreateTexture2D failed: 0x{:x}", hr);

                    return Err(device::BindError::WrongMemory);
                }

                (resource, layers)
            },
            _ => unimplemented!()
        };

        let mut unordered_access_views = Vec::new();
        
        if image.usage.contains(Usage::TRANSFER_DST) {
            for layer in 0..image.kind.num_layers() {
                for mip in 0..image.mip_levels {
                    let view = ViewInfo {
                        resource,
                        kind: image.kind,
                        flags: image::StorageFlags::empty(),
                        view_kind: image::ViewKind::D2,
                        format: typed_raw_format,
                        range: image::SubresourceRange {
                            aspects: format::Aspects::COLOR,
                            levels: mip..(mip + 1),
                            layers: layer..(layer + 1)
                        }
                    };

                    unordered_access_views.push(self.view_image_as_unordered_access_view(&view).map_err(|_| device::BindError::WrongMemory)?);
                }
            }
        }
        

        let (copy_srv, srv) = if image.usage.contains(image::Usage::TRANSFER_SRC) {
            let mut desc = unsafe { mem::zeroed::<d3d11::D3D11_SHADER_RESOURCE_VIEW_DESC>() };
            desc.Format = typed_raw_format;
            desc.ViewDimension = d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2D;
            // TODO:
            *unsafe{ desc.u.Texture2D_mut() } = d3d11::D3D11_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: image.mip_levels as _,
            };

            let mut copy_srv = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateShaderResourceView(
                    resource,
                    &desc,
                    &mut copy_srv as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                error!("CreateShaderResourceView failed: 0x{:x}", hr);

                return Err(device::BindError::WrongMemory);
            }

            desc.Format = dxgi_format;

            let mut srv = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateShaderResourceView(
                    resource,
                    &desc,
                    &mut srv as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                error!("CreateShaderResourceView failed: 0x{:x}", hr);

                return Err(device::BindError::WrongMemory);
            }

            unsafe { (Some(ComPtr::from_raw(copy_srv)), Some(ComPtr::from_raw(srv))) }
        } else {
            (None, None)
        };

        let mut render_target_views = Vec::new();

        if image.usage.contains(image::Usage::COLOR_ATTACHMENT) ||
           image.usage.contains(image::Usage::TRANSFER_DST)
        {
            for layer in 0..image.kind.num_layers() {
                for mip in 0..image.mip_levels {
                    let view = ViewInfo {
                        resource,
                        kind: image.kind,
                        flags: image::StorageFlags::empty(),
                        view_kind: image::ViewKind::D2,
                        format: dxgi_format,
                        range: image::SubresourceRange {
                            aspects: format::Aspects::COLOR,
                            levels: mip..(mip + 1),
                            layers: layer..(layer + 1)
                        }
                    };

                    render_target_views.push(self.view_image_as_render_target(&view).map_err(|_| device::BindError::WrongMemory)?);
                }
            }
        };

        let internal = InternalImage {
            raw: resource,
            copy_srv,
            srv,
            unordered_access_views,
            render_target_views,
        };

        Ok(Image {
            kind: image.kind,
            usage: image.usage,
            format: image.format,
            storage_flags: image.flags,
            dxgi_format,
            typed_raw_format,
            bytes_per_block: bytes_per_block,
            block_dim: block_dim,
            num_levels: levels as _,
            num_mips: image.mip_levels as _,
            internal,
        })
    }

    fn create_image_view(
        &self,
        image: &Image,
        view_kind: image::ViewKind,
        format: format::Format,
        _swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<ImageView, image::ViewError> {
        let info = ViewInfo {
            resource: image.internal.raw,
            kind: image.kind,
            flags: image.storage_flags,
            view_kind,
            format: conv::map_format(format)
                .ok_or(image::ViewError::BadFormat)?,
            range,
        };

        Ok(ImageView {
            srv_handle: if image.usage.contains(image::Usage::SAMPLED) {
                Some(self.view_image_as_shader_resource(&info)?)
            } else {
                None
            },
            // TODO:
            rtv_handle: if image.usage.contains(image::Usage::COLOR_ATTACHMENT) {
                Some(self.view_image_as_render_target(&info)?)
            } else {
                None
            },
            uav_handle: None,
            dsv_handle: if image.usage.contains(image::Usage::DEPTH_STENCIL_ATTACHMENT) {
                Some(self.view_image_as_depth_stencil(&info)?)
            } else {
                None
            },
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> Sampler {
        let op = match info.comparison {
            Some(_) => d3d11::D3D11_FILTER_REDUCTION_TYPE_COMPARISON,
            None => d3d11::D3D11_FILTER_REDUCTION_TYPE_STANDARD,
        };

        let desc = d3d11::D3D11_SAMPLER_DESC {
            Filter: conv::map_filter(info.min_filter, info.mag_filter, info.mip_filter, op, info.anisotropic),
            AddressU: conv::map_wrapping(info.wrap_mode.0),
            AddressV: conv::map_wrapping(info.wrap_mode.1),
            AddressW: conv::map_wrapping(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.anisotropic {
                image::Anisotropic::Off => 0,
                image::Anisotropic::On(aniso) => aniso as _
            },
            ComparisonFunc: info.comparison.map_or(0, |comp| conv::map_comparison(comp)),
            BorderColor: info.border.into(),
            MinLOD: info.lod_range.start.into(),
            MaxLOD: info.lod_range.end.into(),
        };

        let mut sampler = ptr::null_mut();
        let hr = unsafe {
            self.raw.CreateSamplerState(
                &desc,
                &mut sampler as *mut *mut _ as *mut *mut _
            )
        };

        assert_eq!(true, winerror::SUCCEEDED(hr));

        Sampler {
            sampler_handle: unsafe { ComPtr::from_raw(sampler) }
        }
    }

    fn create_descriptor_pool<I>(
        &self,
        max_sets: usize,
        ranges: I,
    ) -> DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>
    {
        let count = ranges.into_iter().map(|r| {
            let r = r.borrow();
            r.count * match r.ty {
                pso::DescriptorType::CombinedImageSampler => 2,
                _ => 1
            }
        }).sum::<usize>() * max_sets;

        DescriptorPool::with_capacity(count)
    }

    fn create_descriptor_set_layout<I, J>(
        &self, layout_bindings: I, _immutable_samplers: J
    ) -> DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<Sampler>,
    {
        let mut max_binding = 0;
        let mut bindings = Vec::new();

        // convert from DescriptorSetLayoutBinding to our own PipelineBinding, and find the higher
        // binding number in the layout
        for binding in layout_bindings {
            let binding = binding.borrow();

            max_binding = max_binding.max(binding.binding as u32);

            bindings.push(PipelineBinding {
                stage: binding.stage_flags,
                ty: binding.ty,
                binding_range: binding.binding..(binding.binding + 1),
                handle_offset: 0
            });
        }

        // we sort the internal descriptor's handle (the actual dx interface) by some categories to
        // make it easier to group api calls together
        bindings.sort_unstable_by(|a, b| {
            (b.ty as u32).cmp(&(a.ty as u32))
            .then(b.binding_range.start.cmp(&a.binding_range.start))
            .then(a.stage.cmp(&b.stage))
        });

        // we also need to map a binding location to its handle
        let mut offset_mapping = vec![(0u32, pso::DescriptorType::Sampler); (max_binding + 1) as usize];
        let mut offset = 0;
        for mut binding in bindings.iter_mut() {
            offset_mapping[binding.binding_range.start as usize] = (offset, binding.ty);

            binding.handle_offset = offset;
            
            offset += match binding.ty {
                pso::DescriptorType::CombinedImageSampler => 2,
                _ => 1
            };
        }

        DescriptorSetLayout {
            bindings,
            offset_mapping,
            handle_count: offset
        }
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        for write in write_iter {
            let target_binding = write.binding as usize;
            let (handle_offset, _ty) = write.set.offset_mapping[target_binding];

            for descriptor in write.descriptors {
                // spill over the writes onto the next binding
                /*while offset >= bind_info.count {
                    assert_eq!(offset, bind_info.count);
                    target_binding += 1;
                    handle_offset = write.set.offset_mapping[target_binding];
                    offset = 0;
                }*/

                let handle = unsafe { write.set.handles.offset(handle_offset as isize) };

                match *descriptor.borrow() {
                    // TODO: binding range
                    pso::Descriptor::Buffer(buffer, ref _range) => {
                        unsafe { *handle = Descriptor(buffer.internal.raw as *mut _); }
                    }
                    pso::Descriptor::Image(image, _layout) => {
                        unsafe { *handle = Descriptor(image.srv_handle.clone().unwrap().as_raw() as *mut _); }
                    }
                    pso::Descriptor::Sampler(sampler) => {
                        unsafe { *handle = Descriptor(sampler.sampler_handle.as_raw() as *mut _); }
                    }
                    pso::Descriptor::CombinedImageSampler(image, _layout, sampler) => {
                        unsafe { *handle = Descriptor(image.srv_handle.clone().unwrap().as_raw() as *mut _); }
                        unsafe { *(handle.offset(1)) = Descriptor(sampler.sampler_handle.as_raw() as *mut _); }
                    }
                    pso::Descriptor::UniformTexelBuffer(_buffer_view) => {
                    }
                    pso::Descriptor::StorageTexelBuffer(_buffer_view) => {
                    }
                }
            }
        }
    }

    fn copy_descriptor_sets<'a, I>(&self, copy_iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, Backend>>,
    {
        for copy in copy_iter {
            let copy = copy.borrow();

            for offset in 0..copy.count {
                let (dst_handle_offset, dst_ty) = copy.dst_set.offset_mapping[copy.dst_binding as usize + offset];
                let (src_handle_offset, src_ty) = copy.src_set.offset_mapping[copy.src_binding as usize + offset];
                assert_eq!(dst_ty, src_ty);

                let dst_handle = unsafe { copy.dst_set.handles.offset(dst_handle_offset as isize) };
                let src_handle = unsafe { copy.dst_set.handles.offset(src_handle_offset as isize) };

                match dst_ty {
                    pso::DescriptorType::CombinedImageSampler => {
                        unsafe { *dst_handle = *src_handle; }
                        unsafe { *(dst_handle.offset(1)) = *(src_handle.offset(1)); }
                    }
                    _ => {
                        unsafe { *dst_handle = *src_handle; }
                    }
                }
            }
        }
    }

    fn map_memory<R>(&self, memory: &Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        if let Some(ref host_visible) = memory.host_visible {
            let ptr = host_visible.borrow_mut().as_mut_ptr();
            memory.mapped_ptr.replace(Some(ptr));

            Ok(unsafe { ptr.offset(*range.start().unwrap_or(&0) as isize) })
        } else {
            error!("Tried to map non-host visible memory");

            Err(mapping::Error::InvalidAccess)
        }
    }

    fn unmap_memory(&self, memory: &Memory) {
        assert_eq!(memory.host_visible.is_some(), true);

        memory.mapped_ptr.replace(None);
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a Memory, R)>,
        R: RangeArg<u64>,
    {

        // go through every range we wrote to
        for range in ranges.into_iter() {
            let &(memory, ref range) = range.borrow();
            let range = memory.resolve(range);

            memory.flush(&self.context, range);
        }
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a Memory, R)>,
        R: RangeArg<u64>,
    {
        // go through every range we want to read from
        for range in ranges.into_iter() {
            let &(memory, ref range) = range.borrow();
            let range = *range.start().unwrap_or(&0)..*range.end().unwrap_or(&memory.size);

            memory.invalidate(&self.context, range);
        }
    }

    fn create_semaphore(&self) -> Semaphore {
        // TODO:
        Semaphore
    }

    fn create_fence(&self, _signalled: bool) -> Fence {
        // TODO:
        Fence
    }

    fn reset_fence(&self, _fence: &Fence) {
        // TODO:
    }

    fn wait_for_fences<I>(&self, _fences: I, _wait: device::WaitFor, _timeout_ms: u32) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<Fence>,
    {
        // TODO:
        true
    }

    fn get_fence_status(&self, _fence: &Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, memory: Memory) {
        for (_range, internal) in memory.local_buffers.borrow_mut().iter() {
            unsafe {
                (*internal.raw).Release();
                if let Some(srv) = internal.srv {
                    (*srv).Release();
                }
            }
        }
    }

    fn create_query_pool(&self, _query_ty: query::QueryType, _count: u32) -> QueryPool {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _pool: QueryPool) {
        unimplemented!()
    }

    fn destroy_shader_module(&self, _shader_lib: ShaderModule) {
    }

    fn destroy_render_pass(&self, _rp: RenderPass) {
        //unimplemented!()
    }

    fn destroy_pipeline_layout(&self, _layout: PipelineLayout) {
        //unimplemented!()
    }

    fn destroy_graphics_pipeline(&self, _pipeline: GraphicsPipeline) {
    }

    fn destroy_compute_pipeline(&self, _pipeline: ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&self, _fb: Framebuffer) {
        //unimplemented!()
    }

    fn destroy_buffer(&self, _buffer: Buffer) {
    }

    fn destroy_buffer_view(&self, _view: BufferView) {
        unimplemented!()
    }

    fn destroy_image(&self, _image: Image) {
        // TODO:
        // unimplemented!()
    }

    fn destroy_image_view(&self, _view: ImageView) {
        //unimplemented!()
    }

    fn destroy_sampler(&self, _sampler: Sampler) {
    }

    fn destroy_descriptor_pool(&self, _pool: DescriptorPool) {
        //unimplemented!()
    }

    fn destroy_descriptor_set_layout(&self, _layout: DescriptorSetLayout) {
        //unimplemented!()
    }

    fn destroy_fence(&self, _fence: Fence) {
        // unimplemented!()
    }

    fn destroy_semaphore(&self, _semaphore: Semaphore) {
        //unimplemented!()
    }

    fn create_swapchain(
        &self,
        surface: &mut Surface,
        config: hal::SwapchainConfig,
        _old_swapchain: Option<Swapchain>,
        _extent: &window::Extent2D,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        // TODO: use IDXGIFactory2 for >=11.1
        // TODO: this function should be able to fail (Result)?

        use conv::map_format;

        debug!("{:#?}", config);

        let (non_srgb_format, format) = {
            // NOTE: DXGI doesn't allow sRGB format on the swapchain, but
            //       creating RTV of swapchain buffers with sRGB works
            let format = match config.color_format {
                format::Format::Bgra8Srgb => format::Format::Bgra8Unorm,
                format::Format::Rgba8Srgb => format::Format::Rgba8Unorm,
                format => format,
            };

            (map_format(format).unwrap(), map_format(config.color_format).unwrap())
        };

        let mut desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: dxgitype::DXGI_MODE_DESC {
                Width: surface.width,
                Height: surface.height,
                // TODO: should this grab max value of all monitor hz? vsync
                //       will clamp to current monitor anyways?
                RefreshRate: dxgitype::DXGI_RATIONAL {
                    Numerator: 1,
                    Denominator: 60
                },
                Format: non_srgb_format,
                ScanlineOrdering: dxgitype::DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                Scaling: dxgitype::DXGI_MODE_SCALING_UNSPECIFIED
            },
            // TODO: msaa on backbuffer?
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0
            },
            BufferUsage: dxgitype::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: config.image_count,
            OutputWindow: surface.wnd_handle,
            // TODO:
            Windowed: TRUE,
            // TODO:
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            Flags: 0
        };
        let swapchain = {
            let mut swapchain: *mut IDXGISwapChain = ptr::null_mut();
            let hr = unsafe {
                surface.factory.CreateSwapChain(
                    self.raw.as_raw() as *mut _,
                    &mut desc as *mut _,
                    &mut swapchain as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: return error

            }

            unsafe { ComPtr::from_raw(swapchain) }
        };

        // TODO: for now we clamp to 1 buffer..
        let images = (0..config.image_count.min(1)).map(|i| {
            let mut resource: *mut d3d11::ID3D11Resource = ptr::null_mut();

            let hr = unsafe {
                swapchain.GetBuffer(
                    i as _,
                    &d3d11::ID3D11Resource::uuidof(),
                    &mut resource as *mut *mut _ as *mut *mut _
                )
            };
            assert_eq!(hr, winerror::S_OK);

            let mut desc: d3d11::D3D11_RENDER_TARGET_VIEW_DESC = unsafe { mem::zeroed() };
            desc.Format = format;
            desc.ViewDimension = d3d11::D3D11_RTV_DIMENSION_TEXTURE2D;
            // NOTE: the rest of the desc should be fine (zeroed)

            let mut rtv = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateRenderTargetView(
                    resource,
                    &desc,
                    &mut rtv as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: error
            }

            let format_desc = config
                .color_format
                .surface_desc();

            let bytes_per_block = (format_desc.bits / 8) as _;
            let block_dim = format_desc.dim;

            let kind = image::Kind::D2(surface.width, surface.height, 1, 1);

            let internal = InternalImage {
                raw: resource,
                copy_srv: None,
                srv: None,
                unordered_access_views: Vec::new(),
                render_target_views: vec![unsafe { ComPtr::from_raw(rtv) }]
            };

            Image {
                kind,
                usage: config.image_usage,
                format: config.color_format,
                storage_flags: image::StorageFlags::empty(),
                // NOTE: not the actual format of the backbuffer(s)
                typed_raw_format: dxgiformat::DXGI_FORMAT_UNKNOWN,
                dxgi_format: format,
                bytes_per_block,
                block_dim,
                num_levels: 1,
                num_mips: 1,
                internal
            }
        }).collect();

        (Swapchain { dxgi_swapchain: swapchain }, hal::Backbuffer::Images(images))
    }

    fn destroy_swapchain(&self, _swapchain: Swapchain) {
        unimplemented!()
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }

}
