//#[deny(missing_docs)]

//#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate derivative;
extern crate gfx_hal as hal;
//#[macro_use]
extern crate log;
extern crate smallvec;
extern crate spirv_cross;
extern crate winapi;
#[cfg(feature = "winit")]
extern crate winit;
extern crate wio;

use hal::{buffer, command, device, error, format, image, memory, mapping, query, pool, pso, pass, Features, Limits, QueueType};
use hal::{DrawCount, IndexCount, InstanceCount, VertexCount, VertexOffset, WorkGroupCount};
use hal::queue::{QueueFamilyId, Queues};
use hal::backend::RawQueueGroup;
use hal::range::RangeArg;

use winapi::shared::{dxgiformat, dxgitype, winerror};

use winapi::shared::dxgi::{DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD, IDXGIFactory, IDXGIAdapter, IDXGISwapChain};
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{GetClientRect};
use winapi::um::{d3d11, d3d11sdklayers, d3dcommon};

use wio::com::ComPtr;

use std::ptr;
use std::mem;
use std::ops::Range;
use std::cell::RefCell;
use std::borrow::{BorrowMut, Borrow};
use std::collections::BTreeMap;

use std::os::raw::c_void;

mod conv;
mod dxgi;
mod shader;

pub struct Instance {
    pub(crate) factory: ComPtr<IDXGIFactory>,
    pub(crate) dxgi_version: dxgi::DxgiVersion
}

unsafe impl Send for Instance { }
unsafe impl Sync for Instance { }

impl Instance {
    pub fn create(_: &str, _: u32) -> Self {
        // TODO: get the latest factory we can find

        let (factory, dxgi_version) = dxgi::get_dxgi_factory().unwrap();

        println!("DXGI version: {:?}", dxgi_version);

        Instance {
            factory,
            dxgi_version
        }
    }

    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        let (width, height) = unsafe {
            let mut rect: RECT = mem::zeroed();
            if GetClientRect(hwnd as *mut _, &mut rect as *mut RECT) == 0 {
                panic!("GetClientRect failed");
            }
            ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32)
        };

        Surface {
            factory: self.factory.clone(),
            wnd_handle: hwnd as *mut _,
            width: width,
            height: height,
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::windows::WindowExt;
        self.create_surface_from_hwnd(window.get_hwnd() as *mut _)
    }
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        let mut adapters = Vec::new();
        let mut idx = 0;

        while let Ok((adapter, info)) = dxgi::get_adapter(idx, self.factory.as_raw(), self.dxgi_version) {
            idx += 1;

            use hal::memory::Properties;

            // TODO: we should improve the way memory is managed. we should
            //       give access to DEFAULT, DYNAMIC and STAGING;
            //
            //       roughly this should translate to:
            //
            //       DEFAULT => DEVICE_LOCAL
            //
            //       NOTE: DYNAMIC only offers cpu write, potentially add
            //             a HOST_WRITE_ONLY flag..
            //       DYNAMIC => DEVICE_LOCAL | CPU_VISIBLE
            //
            //       STAGING => CPU_VISIBLE | CPU_CACHED
            let memory_properties = hal::MemoryProperties {
                memory_types: vec![
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL,
                        heap_index: 0,
                    },
                    hal::MemoryType {
                        properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                        heap_index: 0,
                    },
                    hal::MemoryType {
                        properties: Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                        heap_index: 1,
                    },
                ],
                // TODO: would using *VideoMemory and *SystemMemory from
                //       DXGI_ADAPTER_DESC be too optimistic? :)
                memory_heaps: vec![!0, !0]
            };

            let limits = hal::Limits {
                max_texture_size: d3d11::D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION as _,
                max_patch_size: 0, // TODO
                max_viewports: d3d11::D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as _,
                max_compute_group_count: [
                    d3d11::D3D11_CS_THREAD_GROUP_MAX_X,
                    d3d11::D3D11_CS_THREAD_GROUP_MAX_Y,
                    d3d11::D3D11_CS_THREAD_GROUP_MAX_Z
                ],
                max_compute_group_size: [
                    d3d11::D3D11_CS_THREAD_GROUP_MAX_THREADS_PER_GROUP,
                    1,
                    1
                ], // TODO
                max_vertex_input_attribute_offset: 0, // TODO
                max_vertex_input_attributes: 0, // TODO
                max_vertex_input_binding_stride: 0, // TODO
                max_vertex_input_bindings: 0, // TODO
                max_vertex_output_components: 0, // TODO
                min_buffer_copy_offset_alignment: 0,    // TODO
                min_buffer_copy_pitch_alignment: 0,     // TODO
                min_texel_buffer_offset_alignment: 0,   // TODO
                min_uniform_buffer_offset_alignment: 0, // TODO
                min_storage_buffer_offset_alignment: 0, // TODO
                framebuffer_color_samples_count: 0,     // TODO
                framebuffer_depth_samples_count: 0,     // TODO
                framebuffer_stencil_samples_count: 0,   // TODO
                non_coherent_atom_size: 0,              // TODO
            };

            let physical_device = PhysicalDevice {
                adapter,
                // TODO: check for features
                features: hal::Features::empty(),
                limits,
                memory_properties
            };

            println!("{:#?}", info);

            adapters.push(hal::Adapter {
                info,
                physical_device,
                queue_families: vec![QueueFamily]
            });
        }

        adapters
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PhysicalDevice {
    #[derivative(Debug="ignore")]
    adapter: ComPtr<IDXGIAdapter>,
    features: hal::Features,
    limits: hal::Limits,
    memory_properties: hal::MemoryProperties,
}

unsafe impl Send for PhysicalDevice { }
unsafe impl Sync for PhysicalDevice { }

// TODO: does the adapter we get earlier matter for feature level?
fn get_feature_level(adapter: *mut IDXGIAdapter) -> d3dcommon::D3D_FEATURE_LEVEL {
    let requested_feature_levels = [
        d3dcommon::D3D_FEATURE_LEVEL_11_1,
        d3dcommon::D3D_FEATURE_LEVEL_11_0,
        d3dcommon::D3D_FEATURE_LEVEL_10_1,
        d3dcommon::D3D_FEATURE_LEVEL_10_0,
        d3dcommon::D3D_FEATURE_LEVEL_9_3,
        d3dcommon::D3D_FEATURE_LEVEL_9_2,
        d3dcommon::D3D_FEATURE_LEVEL_9_1,
    ];

    let mut feature_level = d3dcommon::D3D_FEATURE_LEVEL_9_1;
    let hr = unsafe {
        d3d11::D3D11CreateDevice(
            adapter,
            d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
            ptr::null_mut(),
            0,
            requested_feature_levels[..].as_ptr(),
            requested_feature_levels.len() as _,
            d3d11::D3D11_SDK_VERSION,
            ptr::null_mut(),
            &mut feature_level as *mut _,
            ptr::null_mut()
        )
    };

    if !winerror::SUCCEEDED(hr) {
        // if there is no 11.1 runtime installed, requesting
        // `D3D_FEATURE_LEVEL_11_1` will return E_INVALIDARG so we just retry
        // without that
        if hr == winerror::E_INVALIDARG {
            let hr = unsafe {
                d3d11::D3D11CreateDevice(
                    adapter,
                    d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
                    ptr::null_mut(),
                    0,
                    requested_feature_levels[1..].as_ptr(),
                    (requested_feature_levels.len() - 1) as _,
                    d3d11::D3D11_SDK_VERSION,
                    ptr::null_mut(),
                    &mut feature_level as *mut _,
                    ptr::null_mut()
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: device might not support any feature levels?
                unimplemented!();
            }
        }
    }

    feature_level
}

// TODO: PhysicalDevice
impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(&self, families: &[(&QueueFamily, &[hal::QueuePriority])])
        -> Result<hal::Gpu<Backend>, error::DeviceCreationError>
    {
        let (device, cxt) = {
            let feature_level = get_feature_level(self.adapter.as_raw());
            let mut returned_level = d3dcommon::D3D_FEATURE_LEVEL_9_1;

            // TODO: request debug device only on debug config?
            let mut device = ptr::null_mut();
            let mut cxt = ptr::null_mut();
            let hr = unsafe {
                d3d11::D3D11CreateDevice(
                    self.adapter.as_raw() as *mut _,
                    d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
                    ptr::null_mut(),
                    d3d11::D3D11_CREATE_DEVICE_DEBUG,
                    [feature_level].as_ptr(),
                    1,
                    d3d11::D3D11_SDK_VERSION,
                    &mut device as *mut *mut _ as *mut *mut _,
                    &mut returned_level as *mut _,
                    &mut cxt as *mut *mut _ as *mut *mut _,
                )
            };

            // NOTE: returns error if adapter argument is non-null and driver
            // type is not unknown; or if debug device is requested but not
            // present
            if !winerror::SUCCEEDED(hr) {
                return Err(error::DeviceCreationError::InitializationFailed);
            }

            println!("feature level={:x}", feature_level);

            unsafe { (ComPtr::from_raw(device), ComPtr::from_raw(cxt)) }
        };

        let device = Device::new(device, cxt);

        // TODO: deferred context => 1 cxt/queue?
        let queues = Queues::new(
            families
                .into_iter()
                .map(|&(family, prio)| {
                    assert_eq!(prio.len(), 1);
                    let mut group = RawQueueGroup::new(family.clone());

                    // TODO: multiple queues?
                    let queue = CommandQueue {
                        context: device.context.clone(),
                    };
                    group.add_queue(queue);
                    (QueueFamilyId(0), group)
                })
                .collect()
        );

        Ok(hal::Gpu {
            device,
            queues
        })
    }

    fn format_properties(&self, fmt: Option<format::Format>) -> format::Properties {
        unimplemented!()
    }


    fn image_format_properties(&self, _format: format::Format, dimensions: u8, tiling: image::Tiling, usage: image::Usage, storage_flags: image::StorageFlags) -> Option<image::FormatProperties> {
        unimplemented!()
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        self.memory_properties.clone()
    }

    fn features(&self) -> Features {
        self.features
    }

    fn limits(&self) -> Limits {
        self.limits
    }

}



pub struct Device {
    device: ComPtr<d3d11::ID3D11Device>,
    context: ComPtr<d3d11::ID3D11DeviceContext>
}

unsafe impl Send for Device { }
unsafe impl Sync for Device { }

impl Device {
    fn new(device: ComPtr<d3d11::ID3D11Device>, context: ComPtr<d3d11::ID3D11DeviceContext>) -> Self {
        Device {
            device,
            context
        }
    }

    fn create_rasterizer_state(&self, rasterizer_desc: &pso::Rasterizer) -> Result<ComPtr<d3d11::ID3D11RasterizerState>, pso::CreationError> {
        let mut rasterizer = ptr::null_mut();
        let desc = conv::map_rasterizer_desc(rasterizer_desc);

        let hr = unsafe {
            self.device.CreateRasterizerState(
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
            self.device.CreateBlendState(
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
            self.device.CreateDepthStencilState(
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
            self.device.CreateInputLayout(
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
            self.device.CreateVertexShader(
                blob.GetBufferPointer(),
                blob.GetBufferSize(),
                ptr::null_mut(),
                &mut vs as *mut *mut _ as *mut *mut _
            )
        };

        if !winerror::SUCCEEDED(hr) {
            Ok(unsafe { ComPtr::from_raw(vs) })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_pixel_shader(&self, blob: ComPtr<d3dcommon::ID3DBlob>) -> Result<ComPtr<d3d11::ID3D11PixelShader>, pso::CreationError> {
        let mut ps = ptr::null_mut();

        let hr = unsafe {
            self.device.CreatePixelShader(
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
}

impl hal::Device<Backend> for Device {
    fn allocate_memory(
        &self,
        mem_type: hal::MemoryTypeId,
        size: u64,
    ) -> Result<Memory, device::OutOfMemory> {
        // TODO:
        Ok(Memory {
            properties: memory::Properties::CPU_VISIBLE | memory::Properties::CPU_CACHED,
            buffer: RefCell::new(None),
            size,
        })
    }

    fn create_command_pool(
        &self, family: QueueFamilyId, _create_flags: pool::CommandPoolCreateFlags
    ) -> CommandPool {
        // TODO:
        CommandPool {
            device: self.device.clone()
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
        let depth_stencil_state = if let Some(desc) = desc.depth_stencil {
            Some(self.create_depth_stencil_state(&desc)?)
        } else {
            None
        };

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
           usage.contains(Usage::STORAGE_TEXEL) { bind |= d3d11::D3D11_BIND_SHADER_RESOURCE; }

        // TODO: how to do buffer copies
        // if usage.contains(Usage::TRANSFER_SRC) ||
        //    usage.contains(Usage::TRANSFER_DST) { bind |= d3d11::D3D11_BIND_UNORDERED_ACCESS; }

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
        assert_eq!(0, unbound_buffer.bind & d3d11::D3D11_BIND_SHADER_RESOURCE);
        // TODO: change memory to be capable of more than one buffer?
        // assert_eq!(None, memory.buffer);

        use memory::Properties;

        let (usage, cpu) = if memory.properties == Properties::DEVICE_LOCAL {
            (d3d11::D3D11_USAGE_DEFAULT, 0)
        } else if memory.properties == (Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::CPU_CACHED) {
            // TODO: need read access too
            (d3d11::D3D11_USAGE_DYNAMIC, d3d11::D3D11_CPU_ACCESS_WRITE)
        } else if memory.properties == (Properties::CPU_VISIBLE | Properties::CPU_CACHED) {
            (d3d11::D3D11_USAGE_STAGING, d3d11::D3D11_CPU_ACCESS_READ | d3d11::D3D11_CPU_ACCESS_WRITE)
        } else {
            unimplemented!()
        };

        let desc = d3d11::D3D11_BUFFER_DESC {
            ByteWidth: unbound_buffer.size as _,
            Usage: usage,
            BindFlags: unbound_buffer.bind,
            CPUAccessFlags: cpu,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let mut buffer = ptr::null_mut();
        let hr = unsafe {
            self.device.CreateBuffer(
                &desc,
                ptr::null_mut(),
                &mut buffer as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            let buffer: ComPtr<d3d11::ID3D11Buffer> = unsafe { ComPtr::from_raw(buffer) };
            memory.buffer.replace(Some(buffer.clone()));
            Ok(Buffer {
                buffer: buffer,
                size: unbound_buffer.size
            })
        } else {
            // TODO: any better error?
            Err(device::BindError::WrongMemory)
        }
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
        unimplemented!()
    }

    fn get_image_requirements(&self, _image: &UnboundImage) -> memory::Requirements {
        unimplemented!()
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
        unimplemented!()
    }

    fn create_image_view(
        &self,
        image: &Image,
        view_kind: image::ViewKind,
        format: format::Format,
        _swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<ImageView, image::ViewError> {
        // TODO: move out into separate functions
        // TODO: mip level
        // TODO: handle case where range layers >= 1
        assert_eq!(range.layers, 0..1);
        // TODO: SRV, UAV, DSV?

        // TODO: check for rtv/srv/etc format support
        let format = conv::map_format(format).unwrap();

        let mut desc: d3d11::D3D11_RENDER_TARGET_VIEW_DESC = unsafe { mem::zeroed() };
        desc.Format = format;
        desc.ViewDimension = d3d11::D3D11_RTV_DIMENSION_TEXTURE2D;

        let mut rtv = ptr::null_mut();
        let hr = unsafe {
            self.device.CreateRenderTargetView(
                image.resource,
                &desc,
                &mut rtv as *mut *mut _ as *mut *mut _
            )
        };

        if winerror::SUCCEEDED(hr) {
            Ok(ImageView {
                rtv: unsafe { ComPtr::from_raw(rtv) }
            })
        } else {
            // TODO: better errors
            Err(image::ViewError::Unsupported)
        }
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> Sampler {
        unimplemented!()
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
        unimplemented!()
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
        let buffer = memory.buffer.borrow().clone().unwrap();
        let mut mapped = unsafe { mem::zeroed::<d3d11::D3D11_MAPPED_SUBRESOURCE>() };
        let hr = unsafe {
            self.context.Map(
                buffer.as_raw() as _,
                0,
                // TODO:
                d3d11::D3D11_MAP_WRITE_DISCARD,
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
        let buffer = memory.buffer.borrow().clone().unwrap();
        unsafe {
            self.context.Unmap(
                buffer.as_raw() as _,
                0,
            );
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
        unimplemented!()
    }

    fn destroy_compute_pipeline(&self, pipeline: ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&self, _fb: Framebuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&self, buffer: Buffer) {
        unimplemented!()
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
        unimplemented!()
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
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        // TODO: use IDXGIFactory2 for >=11.1
        // TODO: this function should be able to fail (Result)?

        use conv::map_format;

        println!("{:#?}", config);

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
                    self.device.as_raw() as *mut _,
                    &mut desc as *mut _,
                    &mut swapchain as *mut *mut _ as *mut *mut _
                )
            };

            if !winerror::SUCCEEDED(hr) {
                // TODO: return error

            }

            unsafe { ComPtr::from_raw(swapchain) }
        };

        let images = (0..config.image_count).map(|i| {
            let mut resource: *mut d3d11::ID3D11Resource = ptr::null_mut();

            unsafe {
                swapchain.GetBuffer(
                    i as _,
                    &d3d11::IID_ID3D11Resource,
                    &mut resource as *mut *mut _ as *mut *mut _
                );
            };

            let mut desc: d3d11::D3D11_RENDER_TARGET_VIEW_DESC = unsafe { mem::zeroed() };
            desc.Format = format;
            desc.ViewDimension = d3d11::D3D11_RTV_DIMENSION_TEXTURE2D;
            // NOTE: the rest of the desc should be fine (zeroed)

            let mut rtv = ptr::null_mut();
            let hr = unsafe {
                self.device.CreateRenderTargetView(
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
                rtv: unsafe { ComPtr::from_raw(rtv) }
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

pub struct Surface {
    pub(crate) factory: ComPtr<IDXGIFactory>,
    wnd_handle: HWND,
    width: u32,
    height: u32
}

unsafe impl Send for Surface { }
unsafe impl Sync for Surface { }

impl hal::Surface<Backend> for Surface {
    fn supports_queue_family(&self, queue_family: &QueueFamily) -> bool {
        true
        /*match queue_family {
            &QueueFamily::Present => true,
            _ => false
        }*/
    }

    // TODO: stereo swapchain?
    fn kind(&self) -> image::Kind {
        image::Kind::D2(self.width, self.height, 1, 1)
    }

    fn capabilities_and_formats(&self, _: &PhysicalDevice) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>) {
        let extent = hal::window::Extent2D {
            width: self.width,
            height: self.height,
        };

        // TODO: flip swap effects require dx11.1/windows8
        // NOTE: some swap effects affect msaa capabilities..
        // TODO: _DISCARD swap effects can only have one image?
        let capabilities = hal::SurfaceCapabilities {
            image_count: 1..16, // TODO:
            current_extent: Some(extent),
            extents: extent..extent,
            max_image_layers: 1,
        };

        let formats = vec![
            format::Format::Bgra8Srgb,
            format::Format::Bgra8Unorm,
            format::Format::Rgba8Srgb,
            format::Format::Rgba8Unorm,
            format::Format::A2b10g10r10Unorm,
            format::Format::Rgba16Float,
        ];

        (capabilities, Some(formats))
    }

}

pub struct Swapchain {
    dxgi_swapchain: ComPtr<IDXGISwapChain>,
}

unsafe impl Send for Swapchain { }
unsafe impl Sync for Swapchain { }

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _sync: hal::FrameSync<Backend>) -> hal::Frame {
        // TODO: non-`_DISCARD` swap effects have more than one buffer, `FLIP`
        //       effects are dxgi 1.3 (w10+?) in which case there is
        //       `GetCurrentBackBufferIndex()` on the swapchain

        hal::Frame::new(0)
    }
}


#[derive(Debug, Clone, Copy)]
pub struct QueueFamily;

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType { QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct CommandQueue {
    #[derivative(Debug="ignore")]
    context: ComPtr<d3d11::ID3D11DeviceContext>
}

unsafe impl Send for CommandQueue { }
unsafe impl Sync for CommandQueue { }

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(&mut self, submission: hal::queue::RawSubmission<Backend, IC>, fence: Option<&Fence>)
    where
        IC: IntoIterator,
        IC::Item: Borrow<CommandBuffer>,
    {
        for cmd_buf in submission.cmd_buffers.into_iter() {
            let cmd_buf = cmd_buf.borrow();
            self.context.ExecuteCommandList(cmd_buf.as_raw_list().as_raw(), FALSE);
        }
    }

    fn present<IS, IW>(&mut self, swapchains: IS, _wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<Semaphore>,
    {
        for swapchain in swapchains {
            unsafe { swapchain.borrow().dxgi_swapchain.Present(1, 0); }
        }
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }

}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct CommandBuffer {
    #[derivative(Debug="ignore")]
    context: ComPtr<d3d11::ID3D11DeviceContext>,
    #[derivative(Debug="ignore")]
    list: Option<ComPtr<d3d11::ID3D11CommandList>>
}

unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {} 

impl CommandBuffer {
    fn create_deferred(device: ComPtr<d3d11::ID3D11Device>) -> Self {
        let mut context: *mut d3d11::ID3D11DeviceContext = ptr::null_mut();
        let hr = unsafe {
            device.CreateDeferredContext(0, &mut context as *mut *mut _ as *mut *mut _)
        };

        CommandBuffer {
            context: unsafe { ComPtr::from_raw(context) },
            list: None
        }
    }

    fn as_raw_list(&self) -> ComPtr<d3d11::ID3D11CommandList> {
        self.list.clone().unwrap().clone()
    }
}

impl hal::command::RawCommandBuffer<Backend> for CommandBuffer {

    fn begin(&mut self, _flags: command::CommandBufferFlags, _info: command::CommandBufferInheritanceInfo<Backend>) {

        // TODO:
    }

    fn finish(&mut self) {
        // TODO:

        let mut list = ptr::null_mut();
        let hr = unsafe { self.context.FinishCommandList(FALSE, &mut list as *mut *mut _ as *mut *mut _) };
        self.list = Some(unsafe { ComPtr::from_raw(list) });
    }

    fn reset(&mut self, _release_resources: bool) {
        unimplemented!()
    }

    fn begin_render_pass<T>(&mut self, render_pass: &RenderPass, framebuffer: &Framebuffer, target_rect: pso::Rect, clear_values: T, _first_subpass: command::SubpassContents)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ClearValueRaw>,
    {
        // TODO: very temp
        let color_views = framebuffer.attachments.iter().map(|a| a.rtv.as_raw()).collect::<Vec<_>>();
        unsafe {
            self.context.OMSetRenderTargets(
                color_views.len() as _,
                color_views.as_ptr(),
                ptr::null_mut(),
            );
        }
        // TODO: begin render pass
        //unimplemented!()
    }

    fn next_subpass(&mut self, _contents: command::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        // TODO: end render pass
        //unimplemented!()
    }

    fn pipeline_barrier<'a, T>(&mut self, _stages: Range<pso::PipelineStage>, _dependencies: memory::Dependencies, barriers: T)
    where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        unimplemented!()
    }

    fn clear_image<T>(&mut self, image: &Image, _: image::Layout, color: command::ClearColorRaw, depth_stencil: command::ClearDepthStencilRaw, subresource_ranges: T)
    where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        // TODO: use a internal program to clear for subregions in the image
        for subresource_range in subresource_ranges {
            let _sub = subresource_range.borrow();
            unsafe {
                self.context.ClearRenderTargetView(
                    image.rtv.as_raw(),
                    &color.float32
                );
            }
        }
    }

    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<command::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        unimplemented!()
    }

    fn resolve_image<T>(&mut self, src: &Image, _src_layout: image::Layout, dst: &Image, _dst_layout: image::Layout, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageResolve>,
    {
        unimplemented!()
    }

    fn blit_image<T>(&mut self, _src: &Image, _src_layout: image::Layout, _dst: &Image, _dst_layout: image::Layout, _filter: image::Filter, _regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageBlit>
    {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<Backend>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, first_binding: u32, vbs: pso::VertexBufferSet<Backend>) {
        let (buffers, offsets): (Vec<*mut d3d11::ID3D11Buffer>, Vec<u32>) = vbs.0.iter()
            .map(|(buf, offset)| (buf.buffer.as_raw(), *offset as u32))
            .unzip();

        // TODO: strides
        let strides = [16u32; 16];

        unsafe {
            self.context.IASetVertexBuffers(
                first_binding,
                buffers.len() as _,
                buffers.as_ptr(),
                strides.as_ptr(),
                offsets.as_ptr(),
            );
        }
    }

    fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        let viewports = viewports.into_iter().map(|v| {
            let v = v.borrow();
            conv::map_viewport(v)
        }).collect::<Vec<_>>();

        unsafe { self.context.RSSetViewports(viewports.len() as _, viewports.as_ptr()); }
    }

    fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        let scissors = scissors.into_iter().map(|s| {
            let s = s.borrow();
            conv::map_rect(s)
        }).collect::<Vec<_>>();

        unsafe { self.context.RSSetScissorRects(scissors.len() as _, scissors.as_ptr()); }
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        unimplemented!()
    }

    fn set_stencil_reference(&mut self, front: pso::StencilValue, back: pso::StencilValue) {
        unimplemented!()
    }

    fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        unimplemented!()
    }

    fn set_line_width(&mut self, width: f32) {
        validate_line_width(width);
    }

    fn set_depth_bias(&mut self, _depth_bias: pso::DepthBias) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &GraphicsPipeline) {
        unsafe {
            self.context.IASetPrimitiveTopology(pipeline.topology);
            self.context.IASetInputLayout(pipeline.input_layout.as_raw());

            self.context.VSSetShader(pipeline.vs.as_raw(), ptr::null_mut(), 0);
            if let Some(ref ps) = pipeline.ps {
                self.context.PSSetShader(ps.as_raw(), ptr::null_mut(), 0);
            }

            self.context.RSSetState(pipeline.rasterizer_state.as_raw());

            // TODO: blend constants
            self.context.OMSetBlendState(pipeline.blend_state.as_raw(), &[1f32; 4], !0);
            if let Some(ref state) = pipeline.depth_stencil_state {
                // TODO stencil
                self.context.OMSetDepthStencilState(state.as_raw(), 0);
            }
        }
    }

    fn bind_graphics_descriptor_sets<'a, T>(&mut self, layout: &PipelineLayout, first_set: usize, sets: T)
    where
        T: IntoIterator,
        T::Item: Borrow<DescriptorSet>,
    {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pipeline: &ComputePipeline) {
        unimplemented!()
    }


    fn bind_compute_descriptor_sets<T>(&mut self, layout: &PipelineLayout, first_set: usize, sets: T)
    where
        T: IntoIterator,
        T::Item: Borrow<DescriptorSet>,
    {
        unimplemented!()
    }

    fn dispatch(&mut self, count: WorkGroupCount) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset) {
        unimplemented!()
    }

    fn fill_buffer<R>(&mut self, buffer: &Buffer, range: R, data: u32)
    where
        R: RangeArg<buffer::Offset>,
    {
        unimplemented!()
    }

    fn update_buffer(&mut self, _buffer: &Buffer, _offset: buffer::Offset, _data: &[u8]) {
        unimplemented!()
    }

    fn copy_buffer<T>(&mut self, src: &Buffer, dst: &Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferCopy>,
    {
        unimplemented!()
    }

    fn copy_image<T>(&mut self, src: &Image, _: image::Layout, dst: &Image, _: image::Layout, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageCopy>,
    {
        unimplemented!()
    }

    fn copy_buffer_to_image<T>(&mut self, buffer: &Buffer, image: &Image, _: image::Layout, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn copy_image_to_buffer<T>(&mut self, image: &Image, _: image::Layout, buffer: &Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        unsafe {
            self.context.DrawInstanced(
                vertices.end - vertices.start,
                instances.end - instances.start,
                vertices.start,
                instances.start,
            );
        }
    }

    fn draw_indexed(&mut self, indices: Range<IndexCount>, base_vertex: VertexOffset, instances: Range<InstanceCount>) {
        unsafe {
            self.context.DrawIndexedInstanced(
                indices.end - indices.start,
                instances.end - instances.start,
                indices.start,
                base_vertex,
                instances.start,
            );
        }
    }

    fn draw_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset, draw_count: DrawCount, stride: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self, buffer: &Buffer, offset: buffer::Offset, draw_count: DrawCount, stride: u32) {
        unimplemented!()
    }

    fn begin_query(&mut self, query: query::Query<Backend>, flags: query::QueryControl) {
        unimplemented!()
    }

    fn end_query(&mut self, query: query::Query<Backend>) {
        unimplemented!()
    }

    fn reset_query_pool(&mut self, _pool: &QueryPool, _queries: Range<query::QueryId>) {
        unimplemented!()
    }

    fn write_timestamp(&mut self, _: pso::PipelineStage, query: query::Query<Backend>) {
        unimplemented!()
    }

    fn push_graphics_constants(&mut self, layout: &PipelineLayout, _stages: pso::ShaderStageFlags, offset: u32, constants: &[u32]) {
        unimplemented!()
    }

    fn push_compute_constants(&mut self, layout: &PipelineLayout, offset: u32, constants: &[u32]) {
        unimplemented!()
    }

    fn execute_commands<I>(&mut self, buffers: I)
    where
        I: IntoIterator,
        I::Item: Borrow<CommandBuffer>,
    {
        unimplemented!()
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Memory {
    properties: memory::Properties,
    #[derivative(Debug="ignore")]
    // TODO: :-(
    buffer: RefCell<Option<ComPtr<d3d11::ID3D11Buffer>>>,
    size: u64,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {} 

pub struct CommandPool {
    device: ComPtr<d3d11::ID3D11Device>
}

unsafe impl Send for CommandPool {}
unsafe impl Sync for CommandPool {} 

impl hal::pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {

        //unimplemented!()
    }

    fn allocate(&mut self, num: usize, level: command::RawLevel) -> Vec<CommandBuffer> {
        (0..num)
            .map(|_| CommandBuffer::create_deferred(self.device.clone()))
            .collect()
    }

    unsafe fn free(&mut self, _cbufs: Vec<CommandBuffer>) {
        unimplemented!()
    }
}

/// Similarily to dx12 backend, we can handle either precompiled dxbc or spirv
// TODO: derivative doesn't work on enum variants?
//#[derive(Derivative)]
//#[derivative(Debug)]
pub enum ShaderModule {
    Dxbc(Vec<u8>),
    Spirv(Vec<u8>)
}

// TODO: temporary
impl ::std::fmt::Debug for ShaderModule {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", "ShaderModule { ... }")
    }
}

unsafe impl Send for ShaderModule { }
unsafe impl Sync for ShaderModule { }

#[derive(Debug)]
pub struct RenderPass;
#[derive(Debug)]
pub struct Framebuffer {
    attachments: Vec<ImageView>,
    layers: image::Layer,
}

#[derive(Debug)]
pub struct UnboundBuffer {
    usage: buffer::Usage,
    bind: d3d11::D3D11_BIND_FLAG,
    size: u64,
    requirements: memory::Requirements,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer {
    #[derivative(Debug="ignore")]
    buffer: ComPtr<d3d11::ID3D11Buffer>,
    size: u64,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {} 

#[derive(Debug)]
pub struct BufferView;
#[derive(Debug)]
pub struct UnboundImage;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Image {
    #[derivative(Debug="ignore")]
    resource: *mut d3d11::ID3D11Resource,
    kind: image::Kind,
    usage: image::Usage,
    storage_flags: image::StorageFlags,
    dxgi_format: dxgiformat::DXGI_FORMAT,
    bytes_per_block: u8,
    block_dim: (u8, u8),
    num_levels: image::Level,
    #[derivative(Debug="ignore")]
    rtv: ComPtr<d3d11::ID3D11RenderTargetView>
}

unsafe impl Send for Image { }
unsafe impl Sync for Image { }

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct ImageView {
    #[derivative(Debug="ignore")]
    rtv: ComPtr<d3d11::ID3D11RenderTargetView>
}

unsafe impl Send for ImageView { }
unsafe impl Sync for ImageView { }

#[derive(Debug)]
pub struct Sampler;

#[derive(Debug)]
pub struct ComputePipeline;

/// NOTE: some objects are hashed internally and reused when created with the
///       same params[0], need to investigate which interfaces this applies
///       to.
///
/// [0]: https://msdn.microsoft.com/en-us/library/windows/desktop/ff476500(v=vs.85).aspx
#[derive(Derivative)]
#[derivative(Debug)]
pub struct GraphicsPipeline {
    // TODO: gs, hs, ds
    #[derivative(Debug="ignore")]
    vs: ComPtr<d3d11::ID3D11VertexShader>,
    #[derivative(Debug="ignore")]
    ps: Option<ComPtr<d3d11::ID3D11PixelShader>>,
    #[derivative(Debug="ignore")]
    topology: d3d11::D3D11_PRIMITIVE_TOPOLOGY,
    #[derivative(Debug="ignore")]
    input_layout: ComPtr<d3d11::ID3D11InputLayout>,
    #[derivative(Debug="ignore")]
    rasterizer_state: ComPtr<d3d11::ID3D11RasterizerState>,
    #[derivative(Debug="ignore")]
    blend_state: ComPtr<d3d11::ID3D11BlendState>,
    #[derivative(Debug="ignore")]
    depth_stencil_state: Option<ComPtr<d3d11::ID3D11DepthStencilState>>,
}

unsafe impl Send for GraphicsPipeline { }
unsafe impl Sync for GraphicsPipeline { }

#[derive(Debug)]
pub struct PipelineLayout;

#[derive(Debug)]
pub struct DescriptorSetLayout {
    bindings: Vec<pso::DescriptorSetLayoutBinding>,
}

// TODO: descriptor pool
#[derive(Debug)]
pub struct DescriptorPool;
impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(&mut self, layout: &DescriptorSetLayout) -> Result<DescriptorSet, pso::AllocationError> {
        // TODO: descriptor set

        Ok(DescriptorSet)
    }

    fn free_sets(&mut self, descriptor_sets: &[DescriptorSet]) {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct DescriptorSet;

#[derive(Debug)]
pub struct Fence;
#[derive(Debug)]
pub struct Semaphore;
#[derive(Debug)]
pub struct QueryPool;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = CommandQueue;
    type CommandBuffer = CommandBuffer;

    type Memory = Memory;
    type CommandPool = CommandPool;

    type ShaderModule = ShaderModule;
    type RenderPass = RenderPass;
    type Framebuffer = Framebuffer;

    type UnboundBuffer = UnboundBuffer;
    type Buffer = Buffer;
    type BufferView = BufferView;
    type UnboundImage = UnboundImage;
    type Image = Image;
    type ImageView = ImageView;
    type Sampler = Sampler;

    type ComputePipeline = ComputePipeline;
    type GraphicsPipeline = GraphicsPipeline;
    type PipelineLayout = PipelineLayout;
    type DescriptorSetLayout = DescriptorSetLayout;
    type DescriptorPool = DescriptorPool;
    type DescriptorSet = DescriptorSet;

    type Fence = Fence;
    type Semaphore = Semaphore;
    type QueryPool = QueryPool;
}

fn validate_line_width(width: f32) {
    // Note from the Vulkan spec:
    // > If the wide lines feature is not enabled, lineWidth must be 1.0
    // Simply assert and no-op because DX11 never exposes `Features::LINE_WIDTH`
    assert_eq!(width, 1.0);
}
