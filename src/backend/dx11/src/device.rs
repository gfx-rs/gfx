use std::{cmp, ptr, slice};
use std::collections::BTreeMap as Map;
use std::os::raw::c_void;
use std::sync::Arc;
use winapi;
use core::{self, device as d, buffer, texture, mapping};
use core::memory::{self, Bind, Typed};
use core::handle::{self as h, Producer};
use {Resources as R, Share, Buffer, Fence, Texture, Pipeline, Program, Shader};
use command::RawCommandBuffer;
use {CommandList, DeferredContext, ShaderModel};
use native;
use wio::com::ComPtr;


#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MappingGate {
    pointer: *mut c_void,
}

unsafe impl Send for MappingGate {}
unsafe impl Sync for MappingGate {}

impl core::mapping::Gate<R> for MappingGate {
    unsafe fn set<T>(&self, index: usize, val: T) {
        *(self.pointer as *mut T).offset(index as isize) = val;
    }

    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T] {
        slice::from_raw_parts(self.pointer as *const T, len)
    }

    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T] {
        slice::from_raw_parts_mut(self.pointer as *mut T, len)
    }
}

#[derive(Debug)]
struct TextureParam {
    levels: winapi::UINT,
    format: winapi::DXGI_FORMAT,
    bytes_per_texel: winapi::UINT,
    bind: winapi::D3D11_BIND_FLAG,
    usage: winapi::D3D11_USAGE,
    cpu_access: winapi::D3D11_CPU_ACCESS_FLAG,
}

pub struct Device {
    device: ComPtr<winapi::ID3D11Device>,
    share: Arc<Share>,
    frame_handles: h::Manager<R>,
    vs_cache: Map<u64, Vec<u8>>,
    /// Create typed surface formats for the textures. This is useful for debugging
    /// with PIX, since it doesn't understand typeless formats. This may also prevent
    /// some valid views to be created because the typed formats can't be reinterpret.
    use_texture_format_hint: bool,
    sub_data_array: Vec<winapi::D3D11_SUBRESOURCE_DATA>,
    feature_level: winapi::D3D_FEATURE_LEVEL,
}

impl Clone for Device {
    fn clone(&self) -> Device {
        Device::new(self.device.clone(), self.feature_level, self.share.clone())
    }
}

impl Device {
    /// Create a new `Device`.
    pub fn new(device: ComPtr<winapi::ID3D11Device>, feature_level: winapi::D3D_FEATURE_LEVEL, share: Arc<Share>) -> Device {
        Device {
            device: device,
            share: share,
            frame_handles: h::Manager::new(),
            vs_cache: Map::new(),
            use_texture_format_hint: false,
            sub_data_array: Vec::new(),
            feature_level: feature_level,
        }
    }

    #[doc(hidden)]
    pub fn wrap_back_buffer(&mut self, back_buffer: *mut winapi::ID3D11Texture2D, info: texture::Info,
                            desc: texture::RenderDesc) -> h::RawRenderTargetView<R> {
        use core::Device;
        let raw_tex = Texture(native::Texture::D2(back_buffer));
        let color_tex = self.share.handles.borrow_mut().make_texture(raw_tex, info);
        self.view_texture_as_render_target_raw(&color_tex, desc).unwrap()
    }

    /// Return the maximum supported shader model.
    pub fn shader_model(&self) -> ShaderModel {
        match self.feature_level {
            winapi::D3D_FEATURE_LEVEL_10_0 => 40,
            winapi::D3D_FEATURE_LEVEL_10_1 => 41,
            winapi::D3D_FEATURE_LEVEL_11_0 => 50,
            winapi::D3D_FEATURE_LEVEL_11_1 => 51,
            _ => {
                error!("Unknown feature level {:?}", self.feature_level);
                0
            },
        }
    }

    pub fn create_command_buffer(&self) -> RawCommandBuffer<CommandList> {
        CommandList::new().into()
    }

    pub fn create_command_buffer_native(&mut self) -> RawCommandBuffer<DeferredContext> {
        let mut dc = unsafe { ComPtr::<winapi::ID3D11DeviceContext>::new(ptr::null_mut()) };
        let hr = unsafe {
            self.device.CreateDeferredContext(0, &mut dc.as_mut() as *mut &mut _ as *mut *mut _)
        };
        if winapi::SUCCEEDED(hr) {
            DeferredContext::new(dc).into()
        }else {
            panic!("Failed to create a deferred context")
        }
    }

    fn create_buffer_internal(&mut self, info: buffer::Info, raw_data: Option<*const c_void>)
                              -> Result<h::RawBuffer<R>, buffer::CreationError> {
        use winapi::d3d11::*;
        use data::{map_bind, map_usage};

        let (subind, size) = match info.role {
            buffer::Role::Vertex   =>
                (D3D11_BIND_VERTEX_BUFFER, info.size),
            buffer::Role::Index    => {
                if info.stride != 2 && info.stride != 4 {
                    error!("Only U16 and U32 index buffers are allowed");
                    return Err(buffer::CreationError::Other);
                }
                (D3D11_BIND_INDEX_BUFFER, info.size)
            },
            buffer::Role::Constant  => // 16 bit alignment
                (D3D11_BIND_CONSTANT_BUFFER, (info.size + 0xF) & !0xF),
            buffer::Role::Staging =>
                (D3D11_BIND_FLAG(0), info.size)
        };

        assert!(size >= info.size);
        let (usage, cpu) = map_usage(info.usage, info.bind);
        let bind = map_bind(info.bind) | subind;
        if info.bind.contains(memory::RENDER_TARGET) | info.bind.contains(memory::DEPTH_STENCIL) {
            return Err(buffer::CreationError::UnsupportedBind(info.bind))
        }
        let native_desc = D3D11_BUFFER_DESC {
            ByteWidth: size as winapi::UINT,
            Usage: usage,
            BindFlags: bind.0,
            CPUAccessFlags: cpu.0,
            MiscFlags: 0,
            StructureByteStride: 0, //TODO
        };
        let mut sub = D3D11_SUBRESOURCE_DATA {
            pSysMem: ptr::null(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let sub_raw = match raw_data {
            Some(data) => {
                sub.pSysMem = data;
                &sub as *const _
            },
            None => ptr::null(),
        };

        debug!("Creating Buffer with info {:?} and sub-data {:?}", info, sub);
        let mut raw_buf = native::Buffer(ptr::null_mut());
        let hr = unsafe {
            self.device.CreateBuffer(&native_desc, sub_raw, &mut raw_buf.0)
        };
        if winapi::SUCCEEDED(hr) {
            let buf = Buffer(raw_buf);

            use core::memory::Usage::*;
            let mapping = match info.usage {
                Data | Dynamic => None,
                Upload | Download => Some(MappingGate { pointer: ptr::null_mut() }),
            };

            Ok(self.share.handles.borrow_mut().make_buffer(buf, info, mapping))
        } else {
            error!("Failed to create a buffer with desc {:#?}, error {:x}", native_desc, hr);
            Err(buffer::CreationError::Other)
        }
    }

    fn update_sub_data(&mut self, w: texture::Size, h: texture::Size, bpt: winapi::UINT)
                       -> *const winapi::D3D11_SUBRESOURCE_DATA {
        use winapi::UINT;
        for sub in self.sub_data_array.iter_mut() {
            sub.SysMemPitch = w as UINT * bpt;
            sub.SysMemSlicePitch = (h as UINT) * sub.SysMemPitch;
        }
        self.sub_data_array.as_ptr()
    }

    fn create_texture_1d(&mut self, size: texture::Size, array: texture::Layer,
                         tp: TextureParam, misc: winapi::D3D11_RESOURCE_MISC_FLAG)
                         -> Result<native::Texture, winapi::HRESULT>
    {
        use winapi::UINT;
        let native_desc = winapi::D3D11_TEXTURE1D_DESC {
            Width: size as UINT,
            MipLevels: tp.levels,
            ArraySize: array as UINT,
            Format: tp.format,
            Usage: tp.usage,
            BindFlags: tp.bind.0,
            CPUAccessFlags: tp.cpu_access.0,
            MiscFlags: misc.0,
        };
        let sub_data = if self.sub_data_array.len() > 0 {
            let num_data = array as usize * cmp::max(1, tp.levels) as usize;
            if num_data != self.sub_data_array.len() {
                error!("Texture1D with {} slices and {} levels is given {} data chunks",
                    array, tp.levels, self.sub_data_array.len());
                return Err(winapi::S_OK)
            }
            self.update_sub_data(size, 0, tp.bytes_per_texel)
        }else {
            ptr::null()
        };

        debug!("Creating Texture1D with size {:?}, layer {}, param {:?}, and sub-data {:p}",
            size, array, tp, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateTexture1D(&native_desc, sub_data, &mut raw)
        };
        if winapi::SUCCEEDED(hr) {
            Ok(native::Texture::D1(raw))
        }else {
            error!("CreateTexture1D failed on {:#?} with error {:x}", native_desc, hr);
            Err(hr)
        }
    }

    fn create_texture_2d(&mut self, size: [texture::Size; 2], array: texture::Layer, aa: texture::AaMode,
                         tp: TextureParam, misc: winapi::D3D11_RESOURCE_MISC_FLAG)
                         -> Result<native::Texture, winapi::HRESULT>
    {
        use winapi::UINT;
        use data::map_anti_alias;

        let native_desc = winapi::D3D11_TEXTURE2D_DESC {
            Width: size[0] as UINT,
            Height: size[1] as UINT,
            MipLevels: tp.levels,
            ArraySize: array as UINT,
            Format: tp.format,
            SampleDesc: map_anti_alias(aa),
            Usage: tp.usage,
            BindFlags: tp.bind.0,
            CPUAccessFlags: tp.cpu_access.0,
            MiscFlags: misc.0,
        };
        let sub_data = if self.sub_data_array.len() > 0 {
            let num_data = array as usize * cmp::max(1, tp.levels) as usize;
            if num_data != self.sub_data_array.len() {
                error!("Texture2D with {} slices and {} levels is given {} data chunks",
                    array, tp.levels, self.sub_data_array.len());
                return Err(winapi::S_OK)
            }
            self.update_sub_data(size[0], size[1], tp.bytes_per_texel)
        }else {
            ptr::null()
        };

        debug!("Creating Texture2D with size {:?}, layer {}, param {:?}, and sub-data {:p}",
            size, array, tp, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateTexture2D(&native_desc, sub_data, &mut raw)
        };
        if winapi::SUCCEEDED(hr) {
            Ok(native::Texture::D2(raw))
        }else {
            error!("CreateTexture2D failed on {:#?} with error {:x}", native_desc, hr);
            Err(hr)
        }
    }

    fn create_texture_3d(&mut self, size: [texture::Size; 3],
                         tp: TextureParam, misc: winapi::D3D11_RESOURCE_MISC_FLAG)
                         -> Result<native::Texture, winapi::HRESULT>
    {
        use winapi::UINT;
        let native_desc = winapi::D3D11_TEXTURE3D_DESC {
            Width: size[0] as UINT,
            Height: size[1] as UINT,
            Depth: size[2] as UINT,
            MipLevels: tp.levels,
            Format: tp.format,
            Usage: tp.usage,
            BindFlags: tp.bind.0,
            CPUAccessFlags: tp.cpu_access.0,
            MiscFlags: misc.0,
        };
        let sub_data = if self.sub_data_array.len() > 0 {
            if cmp::max(1, tp.levels) as usize != self.sub_data_array.len() {
                error!("Texture3D with {} levels is given {} data chunks",
                    tp.levels, self.sub_data_array.len());
                return Err(winapi::S_OK)
            }
            self.update_sub_data(size[0], size[1], tp.bytes_per_texel)
        }else {
            ptr::null()
        };

        debug!("Creating Texture3D with size {:?}, param {:?}, and sub-data {:p}",
            size, tp, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateTexture3D(&native_desc, sub_data, &mut raw)
        };
        if winapi::SUCCEEDED(hr) {
            Ok(native::Texture::D3(raw))
        }else {
            error!("CreateTexture3D failed on {:#?} with error {:x}", native_desc, hr);
            Err(hr)
        }
    }

    pub fn cleanup(&mut self) {
        self.frame_handles.clear();
    }
}

impl core::Device<R> for Device {
    fn get_capabilities(&self) -> &core::Capabilities {
        &self.share.capabilities
    }

    fn create_buffer_raw(&mut self, info: buffer::Info) -> Result<h::RawBuffer<R>, buffer::CreationError> {
        self.create_buffer_internal(info, None)
    }

    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, role: buffer::Role, bind: Bind)
                                -> Result<h::RawBuffer<R>, buffer::CreationError> {
        let info = buffer::Info {
            role: role,
            usage: memory::Usage::Data,
            bind: bind,
            size: data.len(),
            stride: stride,
        };
        self.create_buffer_internal(info, Some(data.as_ptr() as *const c_void))
    }

    fn create_shader(&mut self, stage: core::shade::Stage, code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        use winapi::ID3D11DeviceChild;
        use core::shade::{CreateShaderError, Stage};
        use mirror::reflect_shader;

        let dev = &mut self.device;
        let len = code.len() as winapi::SIZE_T;
        let (hr, object) = match stage {
            Stage::Vertex => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    dev.CreateVertexShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            Stage::Hull => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    dev.CreateHullShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            Stage::Domain => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    dev.CreateDomainShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            Stage::Geometry => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    dev.CreateGeometryShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            Stage::Pixel => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    dev.CreatePixelShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            //_ => return Err(CreateShaderError::StageNotSupported(stage))
        };

        if winapi::SUCCEEDED(hr) {
            let reflection = reflect_shader(code);
            let hash = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                code.hash(&mut hasher);
                hasher.finish()
            };
            if stage == Stage::Vertex {
                self.vs_cache.insert(hash, code.to_owned());
            }
            let shader = Shader {
                object: object,
                reflection: reflection,
                code_hash: hash,
            };
            Ok(self.share.handles.borrow_mut().make_shader(shader))
        }else {
            Err(CreateShaderError::CompilationFailed(format!("code {}", hr)))
        }
    }

    fn create_program(&mut self, shader_set: &core::ShaderSet<R>)
                      -> Result<h::Program<R>, core::shade::CreateProgramError> {
        use winapi::{ID3D11VertexShader, ID3D11HullShader, ID3D11DomainShader, ID3D11GeometryShader, ID3D11PixelShader};
        use core::shade::{ProgramInfo, Stage};
        use mirror::populate_info;

        let mut info = ProgramInfo {
            vertex_attributes: Vec::new(),
            globals: Vec::new(),
            constant_buffers: Vec::new(),
            textures: Vec::new(),
            unordereds: Vec::new(),
            samplers: Vec::new(),
            outputs: Vec::new(),
            output_depth: false,
            knows_outputs: true,
        };
        let fh = &mut self.frame_handles;
        let prog = match shader_set {
            &core::ShaderSet::Simple(ref vs, ref ps) => {
                let (vs, ps) = (vs.reference(fh), ps.reference(fh));
                populate_info(&mut info, Stage::Vertex, vs.reflection);
                populate_info(&mut info, Stage::Pixel,  ps.reflection);
                unsafe { (*vs.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut ID3D11VertexShader,
                    hs: ptr::null_mut(),
                    ds: ptr::null_mut(),
                    gs: ptr::null_mut(),
                    ps: ps.object as *mut ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            },
            &core::ShaderSet::Geometry(ref vs, ref gs, ref ps) => {
                let (vs, gs, ps) = (vs.reference(fh), gs.reference(fh), ps.reference(fh));
                populate_info(&mut info, Stage::Vertex,   vs.reflection);
                populate_info(&mut info, Stage::Geometry, gs.reflection);
                populate_info(&mut info, Stage::Pixel,    ps.reflection);
                unsafe { (*vs.object).AddRef(); (*gs.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut ID3D11VertexShader,
                    hs: ptr::null_mut(),
                    ds: ptr::null_mut(),
                    gs: gs.object as *mut ID3D11GeometryShader,
                    ps: ps.object as *mut ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            },
            &core::ShaderSet::Tessellated(ref vs, ref hs, ref ds, ref ps) => {
                let (vs, hs, ds, ps) = (vs.reference(fh), hs.reference(fh), ds.reference(fh), ps.reference(fh));

                populate_info(&mut info, Stage::Vertex, vs.reflection);
                populate_info(&mut info, Stage::Hull,   hs.reflection);
                populate_info(&mut info, Stage::Domain, ds.reflection);
                populate_info(&mut info, Stage::Pixel,  ps.reflection);
                unsafe { (*vs.object).AddRef(); (*hs.object).AddRef(); (*ds.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut ID3D11VertexShader,
                    hs: hs.object as *mut ID3D11HullShader,
                    ds: ds.object as *mut ID3D11DomainShader,
                    gs: ptr::null_mut(),
                    ps: ps.object as *mut ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            }
        };
        Ok(self.share.handles.borrow_mut().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        use winapi::d3dcommon::*;
        use core::Primitive::*;
        use data::map_format;
        use state;

        let mut layouts = Vec::new();
        let mut charbuf = [0; 256];
        let mut charpos = 0;
        for (attrib, at_desc) in program.get_info().vertex_attributes.iter().zip(desc.attributes.iter()) {
            use winapi::UINT;
            let (bdesc, elem) = match at_desc {
                &Some((buf_id, ref el)) => match desc.vertex_buffers[buf_id as usize] {
                    Some(ref bd) => (bd, el),
                    None => return Err(core::pso::CreationError),
                },
                &None => continue,
            };
            if elem.offset & 1 != 0 {
                error!("Vertex attribute {} must be aligned to 2 bytes, has offset {}",
                    attrib.name, elem.offset);
                return Err(core::pso::CreationError);
            }
            layouts.push(winapi::D3D11_INPUT_ELEMENT_DESC {
                SemanticName: &charbuf[charpos],
                SemanticIndex: 0,
                Format: match map_format(elem.format, false) {
                    Some(fm) => fm,
                    None => {
                        error!("Unable to find DXGI format for {:?}", elem.format);
                        return Err(core::pso::CreationError);
                    }
                },
                InputSlot: attrib.slot as UINT,
                AlignedByteOffset: elem.offset as UINT,
                InputSlotClass: if bdesc.rate == 0 {
                    winapi::D3D11_INPUT_PER_VERTEX_DATA
                }else {
                    winapi::D3D11_INPUT_PER_INSTANCE_DATA
                },
                InstanceDataStepRate: bdesc.rate as UINT,
            });
            for (out, inp) in charbuf[charpos..].iter_mut().zip(attrib.name.as_bytes().iter()) {
                *out = *inp as i8;
            }
            charpos += attrib.name.as_bytes().len() + 1;
        }

        let prog = *self.frame_handles.ref_program(program);
        let vs_bin = match self.vs_cache.get(&prog.vs_hash) {
            Some(ref code) => &code[..],
            None => {
                error!("VS hash {} is not found in the device cache", prog.vs_hash);
                return Err(core::pso::CreationError);
            }
        };

        let dev = &mut self.device;
        let mut vertex_layout = ptr::null_mut();
        let hr = unsafe {
            dev.CreateInputLayout(
                layouts.as_ptr(), layouts.len() as winapi::UINT,
                vs_bin.as_ptr() as *const c_void, vs_bin.len() as winapi::SIZE_T,
                &mut vertex_layout)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create input layout from {:#?}, error {:x}", layouts, hr);
            return Err(core::pso::CreationError);
        }
        let dummy_dsi = core::pso::DepthStencilInfo { depth: None, front: None, back: None };
        //TODO: cache rasterizer, depth-stencil, and blend states
        let caps = &self.share.capabilities;

        let pso = Pipeline {
            topology: match desc.primitive {
                PointList       => D3D11_PRIMITIVE_TOPOLOGY_POINTLIST,
                LineList        => D3D11_PRIMITIVE_TOPOLOGY_LINELIST,
                LineStrip       => D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP,
                TriangleList    => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                TriangleStrip   => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
                LineListAdjacency        => D3D11_PRIMITIVE_TOPOLOGY_LINELIST_ADJ,
                LineStripAdjacency       => D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ,
                TriangleListAdjacency    => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ,
                TriangleStripAdjacency   => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ,
                PatchList(num)  => {
                    if num == 0 || num > caps.max_patch_size {
                        return Err(core::pso::CreationError)
                    }
                    D3D_PRIMITIVE_TOPOLOGY(D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 + (num as u32) - 1)
                },
            },
            layout: vertex_layout,
            vertex_buffers: desc.vertex_buffers,
            attributes: desc.attributes,
            program: prog,
            rasterizer: state::make_rasterizer(dev, &desc.rasterizer, desc.scissor),
            depth_stencil: state::make_depth_stencil(dev, match desc.depth_stencil {
                Some((_, ref dsi)) => dsi,
                None => &dummy_dsi,
            }),
            blend: state::make_blend(dev, &desc.color_targets),
        };
        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
    }

    fn create_texture_raw(&mut self, desc: texture::Info, hint: Option<core::format::ChannelType>,
                          data_opt: Option<&[&[u8]]>) -> Result<h::RawTexture<R>, texture::CreationError> {
        use core::texture::{AaMode, CreationError, Kind};
        use data::{map_bind, map_usage, map_surface, map_format};

        let (usage, cpu_access) = map_usage(desc.usage, desc.bind);
        let tparam = TextureParam {
            levels: desc.levels as winapi::UINT,
            format: match hint {
                Some(channel) if self.use_texture_format_hint && !desc.bind.contains(memory::DEPTH_STENCIL) => {
                    match map_format(core::format::Format(desc.format, channel), true) {
                        Some(f) => f,
                        None => return Err(CreationError::Format(desc.format, Some(channel)))
                    }
                },
                _ => match map_surface(desc.format) {
                    Some(f) => f,
                    None => return Err(CreationError::Format(desc.format, None))
                },
            },
            bytes_per_texel: (desc.format.get_total_bits() >> 3) as winapi::UINT,
            bind: map_bind(desc.bind),
            usage: usage,
            cpu_access: cpu_access,
        };

        self.sub_data_array.clear();
        if let Some(data) = data_opt {
            for sub in data.iter() {
                self.sub_data_array.push(winapi::D3D11_SUBRESOURCE_DATA {
                    pSysMem: sub.as_ptr() as *const c_void,
                    SysMemPitch: 0,
                    SysMemSlicePitch: 0,
                });
            }
        };
        let misc = if usage != winapi::D3D11_USAGE_IMMUTABLE &&
            desc.bind.contains(memory::RENDER_TARGET | memory::SHADER_RESOURCE) &&
            desc.levels > 1 && data_opt.is_none() {
            winapi::D3D11_RESOURCE_MISC_GENERATE_MIPS
        }else {
            winapi::D3D11_RESOURCE_MISC_FLAG(0)
        };

        let texture_result = match desc.kind {
            Kind::D1(w) =>
                self.create_texture_1d(w, 1, tparam, misc),
            Kind::D1Array(w, d) =>
                self.create_texture_1d(w, d, tparam, misc),
            Kind::D2(w, h, aa) =>
                self.create_texture_2d([w,h], 1, aa, tparam, misc),
            Kind::D2Array(w, h, d, aa) =>
                self.create_texture_2d([w,h], d, aa, tparam, misc),
            Kind::D3(w, h, d) =>
                self.create_texture_3d([w,h,d], tparam, misc),
            Kind::Cube(w) =>
                self.create_texture_2d([w,w], 6*1, AaMode::Single, tparam, misc | winapi::D3D11_RESOURCE_MISC_TEXTURECUBE),
            Kind::CubeArray(w, d) =>
                self.create_texture_2d([w,w], 6*d, AaMode::Single, tparam, misc | winapi::D3D11_RESOURCE_MISC_TEXTURECUBE),
        };

        match texture_result {
            Ok(native) => {
                let tex = Texture(native);
                Ok(self.share.handles.borrow_mut().make_texture(tex, desc))
            },
            Err(_) => Err(CreationError::Kind),
        }
    }

    fn view_buffer_as_shader_resource_raw(&mut self, _hbuf: &h::RawBuffer<R>, _: core::format::Format)
                                      -> Result<h::RawShaderResourceView<R>, d::ResourceViewError> {
        Err(d::ResourceViewError::Unsupported) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                       -> Result<h::RawUnorderedAccessView<R>, d::ResourceViewError> {
        Err(d::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::ResourceDesc)
                                       -> Result<h::RawShaderResourceView<R>, d::ResourceViewError> {
        use winapi::UINT;
        use core::texture::{AaMode, Kind};
        use data::map_format;
        //TODO: support desc.layer parsing

        let (dim, layers, has_levels) = match htex.get_info().kind {
            Kind::D1(_) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE1D, 1, true),
            Kind::D1Array(_, d) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE1DARRAY, d, true),
            Kind::D2(_, _, AaMode::Single) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2D, 1, true),
            Kind::D2(_, _, _) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2DMS, 1, false),
            Kind::D2Array(_, _, d, AaMode::Single) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2DARRAY, d, true),
            Kind::D2Array(_, _, d, _) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2DMSARRAY, d, false),
            Kind::D3(_, _, _) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE3D, 1, true),
            Kind::Cube(_) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURECUBE, 1, true),
            Kind::CubeArray(_, d) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURECUBEARRAY, d, true),
        };

        let format = core::format::Format(htex.get_info().format, desc.channel);
        let native_desc = winapi::D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: match map_format(format, false) {
                Some(fm) => fm,
                None => return Err(d::ResourceViewError::Channel(desc.channel)),
            },
            ViewDimension: dim,
            u: if has_levels {
                assert!(desc.max >= desc.min);
                [desc.min as UINT, (desc.max + 1 - desc.min) as UINT, 0, layers as UINT]
            }else {
                [0, layers as UINT, 0, 0]
            },
        };

        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).as_resource();
        let hr = unsafe {
            (*self.device).CreateShaderResourceView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create SRV from {:#?}, error {:x}", native_desc, hr);
            return Err(d::ResourceViewError::Unsupported);
        }
        Ok(self.share.handles.borrow_mut().make_texture_srv(native::Srv(raw_view), htex))
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &h::RawTexture<R>)
                                        -> Result<h::RawUnorderedAccessView<R>, d::ResourceViewError> {
        Err(d::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::RenderDesc)
                                         -> Result<h::RawRenderTargetView<R>, d::TargetViewError>
    {
        use winapi::UINT;
        use core::texture::{AaMode, Kind};
        use data::map_format;

        let level = desc.level as UINT;
        let (dim, extra) = match (htex.get_info().kind, desc.layer) {
            (Kind::D1(..), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE1D, [level, 0, 0]),
            (Kind::D1Array(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE1DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D1Array(_, nlayers), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE1DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2(_, _, AaMode::Single), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2D, [level, 0, 0]),
            (Kind::D2(_, _, _), None) if level == 0 =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DMS, [0, 0, 0]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D2Array(_, _, nlayers, _), None) if level == 0 =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY, [0, nlayers as UINT, 0]),
            (Kind::D2Array(_, _, nlayers, _), Some(lid)) if level == 0 && lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY, [lid as UINT, 1+lid as UINT, 0]),
            (Kind::D3(_, _, depth), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE3D, [level, 0, depth as UINT]),
            (Kind::D3(_, _, depth), Some(lid)) if lid < depth =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE3D, [level, lid as UINT, 1+lid as UINT]),
            (Kind::Cube(..), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6]),
            (Kind::Cube(..), Some(lid)) if lid < 6 =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::CubeArray(_, nlayers), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6 * nlayers as UINT]),
            (Kind::CubeArray(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 6 * lid as UINT, 6 * (1+lid) as UINT]),
            (_, None) => return Err(d::TargetViewError::Level(desc.level)),
            (_, Some(lid)) => return Err(d::TargetViewError::Layer(texture::LayerError::OutOfBounds(lid, 0))), //TODO
        };
        let format = core::format::Format(htex.get_info().format, desc.channel);
        let native_desc = winapi::D3D11_RENDER_TARGET_VIEW_DESC {
            Format: match map_format(format, true) {
                Some(fm) => fm,
                None => return Err(d::TargetViewError::Channel(desc.channel)),
            },
            ViewDimension: dim,
            u: extra,
        };
        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).as_resource();
        let hr = unsafe {
            (*self.device).CreateRenderTargetView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create RTV from {:#?}, error {:x}", native_desc, hr);
            return Err(d::TargetViewError::Unsupported);
        }
        let size = htex.get_info().kind.level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_rtv(native::Rtv(raw_view), htex, size))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::DepthStencilDesc)
                                         -> Result<h::RawDepthStencilView<R>, d::TargetViewError>
    {
        use winapi::UINT;
        use core::texture::{AaMode, Kind};
        use data::{map_format, map_dsv_flags};

        let level = desc.level as UINT;
        let (dim, extra) = match (htex.get_info().kind, desc.layer) {
            (Kind::D1(..), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE1D, [level, 0, 0]),
            (Kind::D1Array(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE1DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D1Array(_, nlayers), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE1DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2(_, _, AaMode::Single), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2D, [level, 0, 0]),
            (Kind::D2(_, _, _), None) if level == 0 =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DMS, [0, 0, 0]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D2Array(_, _, nlayers, _), None) if level == 0 =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY, [0, nlayers as UINT, 0]),
            (Kind::D2Array(_, _, nlayers, _), Some(lid)) if level == 0 && lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY, [lid as UINT, 1+lid as UINT, 0]),
            (Kind::D3(..), _) => return Err(d::TargetViewError::Unsupported),
            (Kind::Cube(..), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6]),
            (Kind::Cube(..), Some(lid)) if lid < 6 =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::CubeArray(_, nlayers), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6 * nlayers as UINT]),
            (Kind::CubeArray(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 6 * lid as UINT, 6 * (1+lid) as UINT]),
            (_, None) => return Err(d::TargetViewError::Level(desc.level)),
            (_, Some(lid)) => return Err(d::TargetViewError::Layer(texture::LayerError::OutOfBounds(lid, 0))), //TODO
        };

        let channel = core::format::ChannelType::Uint; //doesn't matter
        let format = core::format::Format(htex.get_info().format, channel);
        let native_desc = winapi::D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: match map_format(format, true) {
                Some(fm) => fm,
                None => return Err(d::TargetViewError::Channel(channel)),
            },
            ViewDimension: dim,
            Flags: map_dsv_flags(desc.flags).0,
            u: extra,
        };

        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).as_resource();
        let hr = unsafe {
            (*self.device).CreateDepthStencilView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create DSV from {:#?}, error {:x}", native_desc, hr);
            return Err(d::TargetViewError::Unsupported);
        }
        let dim = htex.get_info().kind.level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_dsv(native::Dsv(raw_view), htex, dim))
    }

    fn create_sampler(&mut self, info: texture::SamplerInfo) -> h::Sampler<R> {
        use core::texture::FilterMethod;
        use data::{FilterOp, map_function, map_filter, map_wrap};

        let op = if info.comparison.is_some() {FilterOp::Comparison} else {FilterOp::Product};
        let native_desc = winapi::D3D11_SAMPLER_DESC {
            Filter: map_filter(info.filter, op),
            AddressU: map_wrap(info.wrap_mode.0),
            AddressV: map_wrap(info.wrap_mode.1),
            AddressW: map_wrap(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.filter {
                FilterMethod::Anisotropic(max) => max as winapi::UINT,
                _ => 0,
            },
            ComparisonFunc: map_function(info.comparison.unwrap_or(core::state::Comparison::Always)),
            BorderColor: info.border.into(),
            MinLOD: info.lod_range.0.into(),
            MaxLOD: info.lod_range.1.into(),
        };

        let mut raw_sampler = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateSamplerState(&native_desc, &mut raw_sampler)
        };
        if winapi::SUCCEEDED(hr) {
            self.share.handles.borrow_mut().make_sampler(native::Sampler(raw_sampler), info)
        } else {
            error!("Unable to create a sampler with desc {:#?}, error {:x}", info, hr);
            unimplemented!()
        }
    }

    fn create_semaphore(&mut self) -> h::Semaphore<R> {
        self.share.handles.borrow_mut().make_semaphore(())
    }

    fn create_fence(&mut self, _signalled: bool) -> h::Fence<R> {
        self.share.handles.borrow_mut().make_fence(Fence)
    }

    fn reset_fences(&mut self, fences: &[&h::Fence<R>]) {
        // TODO: noop?
    }

    fn wait_for_fences(&mut self, _fences: &[&h::Fence<R>], _wait: d::WaitFor, _timeout_ms: u32) -> bool {
        // TODO: noop?
        true
    }

    fn read_mapping<'a, 'b, T>(&'a mut self, buf: &'b h::Buffer<R, T>)
                               -> Result<mapping::Reader<'b, R, T>,
                                         mapping::Error>
        where T: Copy
    {
        unsafe {
            mapping::read(buf.raw(), |mut m| {
                ensure_mapped(&mut m, buf.raw(), winapi::d3d11::D3D11_MAP_READ, self)
            })
        }
    }

    fn write_mapping<'a, 'b, T>(&'a mut self, buf: &'b h::Buffer<R, T>)
                               -> Result<mapping::Writer<'b, R, T>,
                                         mapping::Error>
        where T: Copy
    {
        unsafe {
            mapping::write(buf.raw(), |mut m| {
                // not MAP_WRITE_DISCARD because we are STAGING
                ensure_mapped(&mut m, buf.raw(), winapi::d3d11::D3D11_MAP_WRITE, self)
            })
        }
    }
}

pub fn ensure_mapped(mapping: &mut MappingGate,
                     buffer: &h::RawBuffer<R>,
                     map_type: winapi::d3d11::D3D11_MAP,
                     device: &mut Device) {
    if mapping.pointer.is_null() {
        let raw_handle = *buffer.resource();
        let mut ctx = ptr::null_mut();

        unsafe {
            device.device.GetImmediateContext(&mut ctx);
        }

        let mut sres = winapi::d3d11::D3D11_MAPPED_SUBRESOURCE {
            pData: ptr::null_mut(),
            RowPitch: 0,
            DepthPitch: 0,
        };

        let dst = raw_handle.as_resource() as *mut winapi::d3d11::ID3D11Resource;
        let hr = unsafe {
            (*ctx).Map(dst, 0, map_type, 0, &mut sres)
        };

        if winapi::SUCCEEDED(hr) {
            mapping.pointer = sres.pData;
        } else {
            panic!("Unable to map a buffer {:?}, error {:x}", buffer, hr);
        }
    }
}

pub fn ensure_unmapped(mapping: &mut MappingGate,
                       buffer: &buffer::Raw<R>,
                       context: &mut ComPtr<winapi::ID3D11DeviceContext>) {
    if !mapping.pointer.is_null() {
        let raw_handle = *buffer.resource();
        unsafe {
            context.Unmap(raw_handle.as_resource() as *mut winapi::d3d11::ID3D11Resource, 0);
        }

        mapping.pointer = ptr::null_mut();
    }
}
