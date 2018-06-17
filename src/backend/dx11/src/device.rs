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
    Fence, Framebuffer, GraphicsPipeline, Image, ImageView, InternalBuffer, Memory, PipelineLayout,
    QueryPool, RenderPass, Sampler, Semaphore, ShaderModule, Surface, Swapchain, UnboundBuffer,
    UnboundImage, ViewInfo,
};

use {conv, internal, shader};

pub struct Device {
    raw: ComPtr<d3d11::ID3D11Device>,
    pub(crate) context: ComPtr<d3d11::ID3D11DeviceContext>,
    memory_properties: hal::MemoryProperties,
    pub(crate) internal: internal::BufferImageCopy
}

unsafe impl Send for Device { }
unsafe impl Sync for Device { }

impl Device {
    pub fn new(device: ComPtr<d3d11::ID3D11Device>, context: ComPtr<d3d11::ID3D11DeviceContext>, memory_properties: hal::MemoryProperties) -> Self {
        Device {
            raw: device.clone(),
            context,
            memory_properties,
            internal: internal::BufferImageCopy::new(device)
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

    fn create_depth_stencil_state(&self, depth_desc: &pso::DepthStencilDesc) -> Result<ComPtr<d3d11::ID3D11DepthStencilState>, pso::CreationError> {
        let mut depth = ptr::null_mut();
        let desc = conv::map_depth_stencil_desc(depth_desc);

        let hr = unsafe {
            self.raw.CreateDepthStencilState(
                &desc,
                &mut depth as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(depth) })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_input_layout(&self, vs: ComPtr<d3dcommon::ID3DBlob>, vertex_buffers: &[pso::VertexBufferDesc], attributes: &[pso::AttributeDesc], input_assembler: &pso::InputAssemblerDesc) -> Result<(d3d11::D3D11_PRIMITIVE_TOPOLOGY, ComPtr<d3d11::ID3D11InputLayout>), pso::CreationError> {
        let mut layout = ptr::null_mut();

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

            Ok((topology, unsafe { ComPtr::from_raw(layout) }))
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
            ShaderModule::Dxbc(ref shader) => {
                unimplemented!()

                // Ok(Some(shader))
            }
            ShaderModule::Spirv(ref raw_data) => {
                Ok(shader::compile_spirv_entrypoint(raw_data, stage, source, layout)?)
            }
        }
    }

    fn view_image_as_shader_resource(&self, info: ViewInfo) -> Result<ComPtr<d3d11::ID3D11ShaderResourceView>, image::ViewError> {
        let mut desc: d3d11::D3D11_SHADER_RESOURCE_VIEW_DESC = unsafe { mem::zeroed() };
        desc.Format = info.format;

        let MostDetailedMip = info.range.levels.start as _;
        let MipLevels = (info.range.levels.end - info.range.levels.start) as _;
        // let FirstArraySlice = info.range.layers.start as _;
        // let ArraySize = (info.range.layers.end - info.range.layers.start) as _;

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

    fn view_image_as_render_target(&self, info: ViewInfo) -> Result<ComPtr<d3d11::ID3D11RenderTargetView>, image::ViewError> {
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

    fn view_image_as_depth_stencil(&self, info: ViewInfo) -> Result<ComPtr<d3d11::ID3D11DepthStencilView>, image::ViewError> {
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
        // TODO:
        Ok(Memory {
            properties: self.memory_properties.memory_types[mem_type.0].properties,
            buffer: RefCell::new(None),
            size,
        })
    }

    fn create_command_pool(
        &self, family: QueueFamilyId, _create_flags: pool::CommandPoolCreateFlags
    ) -> CommandPool {
        // TODO:
        CommandPool {
            device: self.raw.clone(),
            internal: self.internal.clone(),
        }
    }

    fn destroy_command_pool(&self, _pool: CommandPool) {
        unimplemented!()
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
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
        sets: IS,
        push_constant_ranges: IR,
    ) -> PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        // TODO: pipelinelayout

        PipelineLayout
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

        let (topology, input_layout) = self.create_input_layout(vs.clone(), &desc.vertex_buffers, &desc.attributes, &desc.input_assembler)?;
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
            depth_stencil_state
        })
    }

    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, Backend>,
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
        mut size: u64,
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
        // TODO: offset
        assert_eq!(0, offset);
        // TODO: structured buffers
        // assert_eq!(0, unbound_buffer.bind & d3d11::D3D11_BIND_SHADER_RESOURCE);
        // TODO: change memory to be capable of more than one buffer?
        // assert_eq!(None, memory.buffer);

        use memory::Properties;

        debug!("usage={:?}, props={:b}", unbound_buffer.usage, memory.properties);
        let MiscFlags = if unbound_buffer.usage.contains(buffer::Usage::TRANSFER_SRC) {
            d3d11::D3D11_RESOURCE_MISC_BUFFER_STRUCTURED
        } else {
            0
        };

        let buffer = if memory.properties == Properties::DEVICE_LOCAL {
            // device local memory
            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: unbound_buffer.size as _,
                Usage: d3d11::D3D11_USAGE_DEFAULT,
                BindFlags: unbound_buffer.bind,
                CPUAccessFlags: 0,
                MiscFlags,
                StructureByteStride: 0,
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
            } else {
                InternalBuffer::Coherent(unsafe { ComPtr::from_raw(buffer) })
            }
        } else if memory.properties == (Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::CPU_CACHED) {
            // coherent device local and cpu-visible memory
            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: unbound_buffer.size as _,
                Usage: d3d11::D3D11_USAGE_DYNAMIC,
                BindFlags: unbound_buffer.bind,
                CPUAccessFlags: d3d11::D3D11_CPU_ACCESS_WRITE,
                MiscFlags,
                StructureByteStride: 0,
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
            } else {
                InternalBuffer::Coherent(unsafe { ComPtr::from_raw(buffer) })
            }
        } else if memory.properties == (Properties::CPU_VISIBLE | Properties::CPU_CACHED) {
            // non-coherent cpu-visible memory, need to create two buffers to
            // allow gpu-read beyond copying
            let staging = {
                let desc = d3d11::D3D11_BUFFER_DESC {
                    ByteWidth: unbound_buffer.size as _,
                    Usage: d3d11::D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: d3d11::D3D11_CPU_ACCESS_READ | d3d11::D3D11_CPU_ACCESS_WRITE,
                    MiscFlags: 0,
                    StructureByteStride: 0,
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
                } else {
                    unsafe { ComPtr::from_raw(buffer) }
                }
            };

            let device = {
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
                } else {
                    unsafe { ComPtr::from_raw(buffer) }
                }
            };

            InternalBuffer::NonCoherent {
                device,
                staging
            }
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
                    buffer.device_local_buffer().as_raw() as *mut _,
                    &desc,
                    &mut srv as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: better errors
                return Err(device::BindError::WrongMemory);
            } else {
                Some(unsafe { ComPtr::from_raw(srv) })
            }
        } else {
            None
        };

        // TODO:
        memory.buffer.replace(Some(buffer.clone()));

        Ok(Buffer {
            buffer,
            srv,
            size: unbound_buffer.size
        })
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        buffer: &Buffer,
        format: Option<format::Format>,
        range: R,
    ) -> Result<BufferView, buffer::ViewError> {
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

        if usage.contains(Usage::COLOR_ATTACHMENT) { bind |= d3d11::D3D11_BIND_RENDER_TARGET; }
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
        offset: u64,
        image: UnboundImage,
    ) -> Result<Image, device::BindError> {
        use memory::Properties;

        let base_format = image.format.base_format();
        let format_desc = base_format.0.desc();
        let bytes_per_block = (format_desc.bits / 8) as _;
        let block_dim = format_desc.dim;
        let extent = image.kind.extent();

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
        let typeless_format = conv::typeless_format(dxgi_format).unwrap();

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
                    // TODO: better errors
                    return Err(device::BindError::WrongMemory);
                }

                (resource, layers)
            },
            _ => unimplemented!()
        };

        let uav = if image.usage.contains(image::Usage::TRANSFER_DST) {
            let mut desc = unsafe { mem::zeroed::<d3d11::D3D11_UNORDERED_ACCESS_VIEW_DESC>() };
            desc.Format = dxgiformat::DXGI_FORMAT_R32_UINT;
            desc.ViewDimension = d3d11::D3D11_UAV_DIMENSION_TEXTURE2D;

            let mut uav = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateUnorderedAccessView(
                    resource,
                    &desc,
                    &mut uav as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: better errors
                return Err(device::BindError::WrongMemory);
            } else {
                Some(unsafe { ComPtr::from_raw(uav) })
            }
        } else {
            None
        };


        let rtv = if image.usage.contains(image::Usage::COLOR_ATTACHMENT) {
            let mut rtv = ptr::null_mut();
            let hr = unsafe {
                self.raw.CreateRenderTargetView(
                    resource,
                    ptr::null_mut(),
                    &mut rtv as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                return Err(device::BindError::WrongMemory);
            } else {
                Some(unsafe { ComPtr::from_raw(rtv) })
            }
        } else {
            None
        };

        Ok(Image {
            resource: resource,
            kind: image.kind,
            usage: image.usage,
            storage_flags: image.flags,
            dxgi_format,
            bytes_per_block: bytes_per_block,
            block_dim: block_dim,
            num_levels: levels as _,
            uav,
            rtv //unsafe { ComPtr::from_raw(rtv) }
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
            resource: image.resource,
            kind: image.kind,
            flags: image.storage_flags,
            view_kind,
            format: conv::map_format(format)
                .ok_or(image::ViewError::BadFormat)?,
            range,
        };

        Ok(ImageView {
            srv_handle: if image.usage.contains(image::Usage::SAMPLED) {
                Some(self.view_image_as_shader_resource(info.clone())?)
            } else {
                None
            },
            // TODO:
            rtv_handle: if image.usage.contains(image::Usage::COLOR_ATTACHMENT) {
                Some(self.view_image_as_render_target(info.clone())?)
            } else {
                None
            },
            uav_handle: None,
            dsv_handle: if image.usage.contains(image::Usage::DEPTH_STENCIL_ATTACHMENT) {
                Some(self.view_image_as_depth_stencil(info.clone())?)
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
        descriptor_pools: I,
    ) -> DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>
    {
        // TODO: descriptor pool

        DescriptorPool
    }

    fn create_descriptor_set_layout<I, J>(
        &self, bindings: I, _immutable_samplers: J
    ) -> DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<Sampler>,
    {
        // TODO: descriptorsetlayout

        DescriptorSetLayout {
            bindings: bindings.into_iter().map(|b| b.borrow().clone()).collect()
        }
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {

        for write in write_iter {
            let mut offset = write.array_offset as u64;
            let mut target_binding = write.binding as usize;
            //let mut bind_info = &write.set.binding_infos[target_binding];
            for descriptor in write.descriptors {
                // spill over the writes onto the next binding
                /*while offset >= bind_info.count {
                    assert_eq!(offset, bind_info.count);
                    target_binding += 1;
                    bind_info = &write.set.binding_infos[target_binding];
                    offset = 0;
                }*/

                debug!("offset={}, target_binding={}", offset, target_binding);
                match *descriptor.borrow() {
                    pso::Descriptor::Buffer(buffer, ref range) => {
                        write.set.cbv_handles.borrow_mut().push((target_binding as _, buffer.device_local_buffer()));
                        debug!("buffer={:#?}, range={:#?}", buffer, range);
                    }
                    pso::Descriptor::Image(image, _layout) => {
                        write.set.srv_handles.borrow_mut().push((target_binding as _, image.srv_handle.clone().unwrap()));
                        debug!("image={:#?}, layout={:#?}", image, _layout);
                    }
                    pso::Descriptor::CombinedImageSampler(image, _layout, sampler) => {
                    }
                    pso::Descriptor::Sampler(sampler) => {
                        write.set.sampler_handles.borrow_mut().push((target_binding as _, sampler.sampler_handle.clone()));
                        debug!("sampler={:#?}", sampler);
                    }
                    pso::Descriptor::UniformTexelBuffer(buffer_view) => {
                    }
                    pso::Descriptor::StorageTexelBuffer(buffer_view) => {
                    }
                }
                offset += 1;
            }
        }
    }

    fn copy_descriptor_sets<'a, I>(&self, copy_iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, Backend>>,
    {
        unimplemented!()
    }

    fn map_memory<R>(&self, memory: &Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        let buffer = match memory.buffer.borrow().clone().unwrap() {
            InternalBuffer::Coherent(buf) => buf,
            InternalBuffer::NonCoherent { device, staging } => staging
        };
        let mut mapped = unsafe { mem::zeroed::<d3d11::D3D11_MAPPED_SUBRESOURCE>() };
        let hr = unsafe {
            self.context.Map(
                buffer.as_raw() as _,
                0,
                // TODO:
                d3d11::D3D11_MAP_WRITE,
                0,
                &mut mapped
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(mapped.pData as _)
        } else {
            // TODO: better error
            Err(mapping::Error::InvalidAccess)
        }
    }

    fn unmap_memory(&self, memory: &Memory) {
        let (buffer, device_buffer) = match memory.buffer.borrow().clone().unwrap() {
            InternalBuffer::Coherent(buf) => (buf, None),
            InternalBuffer::NonCoherent { device, staging } => (staging, Some(device))
        };

        unsafe {
            self.context.Unmap(
                buffer.as_raw() as _,
                0,
            );

            // coherency!
            if let Some(device_buffer) = device_buffer {
                self.context.CopyResource(
                    device_buffer.as_raw() as _,
                    buffer.as_raw() as _,
                );
            }
        }
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a Memory, R)>,
        R: RangeArg<u64>,
    {
        // TODO: flush?
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a Memory, R)>,
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    fn create_semaphore(&self) -> Semaphore {
        // TODO:
        Semaphore
    }

    fn create_fence(&self, signalled: bool) -> Fence {
        // TODO:
        Fence
    }

    fn reset_fence(&self, fence: &Fence) {
        // TODO:
    }

    fn wait_for_fences<I>(&self, fences: I, wait: device::WaitFor, timeout_ms: u32) -> bool
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
        unimplemented!()
    }

    fn create_query_pool(&self, query_ty: query::QueryType, count: u32) -> QueryPool {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _pool: QueryPool) {
        unimplemented!()
    }

    fn destroy_shader_module(&self, shader_lib: ShaderModule) {
    }

    fn destroy_render_pass(&self, _rp: RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&self, layout: PipelineLayout) {
        unimplemented!()
    }

    fn destroy_graphics_pipeline(&self, pipeline: GraphicsPipeline) {
    }

    fn destroy_compute_pipeline(&self, pipeline: ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&self, _fb: Framebuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&self, buffer: Buffer) {
    }

    fn destroy_buffer_view(&self, _view: BufferView) {
        unimplemented!()
    }

    fn destroy_image(&self, image: Image) {
        unimplemented!()
    }

    fn destroy_image_view(&self, _view: ImageView) {
        unimplemented!()
    }

    fn destroy_sampler(&self, _sampler: Sampler) {
    }

    fn destroy_descriptor_pool(&self, pool: DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&self, _layout: DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&self, _fence: Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&self, _semaphore: Semaphore) {
        unimplemented!()
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

            Image {
                resource,
                kind,
                usage: config.image_usage,
                storage_flags: image::StorageFlags::empty(),
                // NOTE: not the actual format of the backbuffer(s)
                dxgi_format: format,
                bytes_per_block,
                block_dim,
                num_levels: 1,
                uav: None,
                rtv: Some(unsafe { ComPtr::from_raw(rtv) })
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
