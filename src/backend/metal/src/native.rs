use {Backend};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::raw::{c_void, c_long, c_int};
use std::ptr;

use hal::{self, image, pass, pso};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::{self, MTLPrimitiveType};
use objc;
use spirv_cross::msl;


/// Shader module can be compiled in advance if it's resource bindings do not
/// depend on pipeline layout, in which case the value would become `Compiled`.
#[derive(Debug)]
pub enum ShaderModule {
    Compiled {
        library: metal::Library,
        remapped_entry_point_names: HashMap<String, String>
    },
    Raw(Vec<u8>),
}

unsafe impl Send for ShaderModule {}
unsafe impl Sync for ShaderModule {}

#[derive(Debug)]
pub struct RenderPass {
    pub(crate) desc: metal::RenderPassDescriptor,
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) num_colors: usize,
}

unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}

#[derive(Debug)]
pub struct FrameBuffer(pub(crate) metal::RenderPassDescriptor);

unsafe impl Send for FrameBuffer {}
unsafe impl Sync for FrameBuffer {}


#[derive(Debug)]
pub struct PipelineLayout {
    // First vertex buffer index to be used by attributes
    pub(crate) attribute_buffer_index: u32,
    pub(crate) res_overrides: HashMap<msl::ResourceBindingLocation, msl::ResourceBinding>,
}

#[derive(Debug)]
pub struct GraphicsPipeline {
    // we hold the compiled libraries here for now
    // TODO: move to some cache in `Device`
    pub(crate) vs_lib: metal::Library,
    pub(crate) fs_lib: Option<metal::Library>,
    pub(crate) raw: metal::RenderPipelineState,
    pub(crate) primitive_type: MTLPrimitiveType,
    pub(crate) attribute_buffer_index: u32,
    pub(crate) depth_stencil_state: Option<metal::DepthStencilState>,
}

unsafe impl Send for GraphicsPipeline {}
unsafe impl Sync for GraphicsPipeline {}

#[derive(Debug)]
pub struct ComputePipeline {}

#[derive(Debug)]
pub struct Image {
    pub(crate) raw: metal::Texture,
    pub(crate) bytes_per_block: u8,
    // Dimension of a texel block (compressed formats).
    pub(crate) block_dim: (u8, u8),
}

unsafe impl Send for Image {}
unsafe impl Sync for Image {}

#[derive(Debug)]
pub struct BufferView {}

#[derive(Debug)]
pub struct ImageView(pub(crate) metal::Texture);

unsafe impl Send for ImageView {}
unsafe impl Sync for ImageView {}

#[derive(Debug)]
pub struct Sampler(pub(crate) metal::SamplerState);

unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

#[derive(Debug)]
pub struct Semaphore(pub(crate) *mut c_void);

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Buffer(pub(crate) metal::Buffer);

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}


#[derive(Debug)]
pub enum DescriptorPool {
    Emulated,
    ArgumentBuffer {
        buffer: metal::Buffer,
        total_size: NSUInteger,
        offset: NSUInteger,
    }
}
//TODO: re-evaluate Send/Sync here
unsafe impl Send for DescriptorPool {}
unsafe impl Sync for DescriptorPool {}

impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        match *self {
            DescriptorPool::Emulated => {
                layouts.iter().map(|layout| {
                    let layout_bindings = match layout {
                        &&DescriptorSetLayout::Emulated(ref bindings) => bindings,
                        _ => panic!("Incompatible descriptor set layout type"),
                    };

                    let bindings = layout_bindings.iter().map(|layout| {
                        let binding = match layout.ty {
                            pso::DescriptorType::Sampler => {
                                let resources = (0 .. layout.count).map(|_| None);
                                DescriptorSetBinding::Sampler(resources.collect())
                            }
                            pso::DescriptorType::SampledImage => {
                                let resources = (0 .. layout.count)
                                    .map(|_| None);
                                DescriptorSetBinding::SampledImage(resources.collect())
                            }
                            pso::DescriptorType::UniformBuffer => {
                                let resources = (0 .. layout.count).map(|_| None);
                                DescriptorSetBinding::ConstantBuffer(resources.collect())
                            }
                            _ => unimplemented!()
                        };
                        (layout.binding, binding)
                    }).collect();

                    let inner = DescriptorSetInner {
                        layout: layout_bindings.to_vec(),
                        bindings,
                    };
                    DescriptorSet::Emulated(Arc::new(Mutex::new(inner)))
                }).collect()
            }
            DescriptorPool::ArgumentBuffer { ref buffer, total_size, ref mut offset } => {
                layouts.iter().map(|layout| {
                    let (encoder, stage_flags) = match layout {
                        &&DescriptorSetLayout::ArgumentBuffer(ref encoder, stages) => (encoder, stages),
                        _ => panic!("Incompatible descriptor set layout type"),
                    };

                    let cur_offset = *offset;
                    *offset += encoder.encoded_length();
                    assert!(*offset <= total_size);

                    DescriptorSet::ArgumentBuffer {
                        buffer: buffer.clone(),
                        offset: cur_offset,
                        encoder: encoder.clone(),
                        stage_flags,
                    }
                }).collect()
            }
        }
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub enum DescriptorSetLayout {
    Emulated(Vec<pso::DescriptorSetLayoutBinding>),
    ArgumentBuffer(metal::ArgumentEncoder, pso::ShaderStageFlags),
}
unsafe impl Send for DescriptorSetLayout {}
unsafe impl Sync for DescriptorSetLayout {}

#[derive(Clone, Debug)]
pub enum DescriptorSet {
    Emulated(Arc<Mutex<DescriptorSetInner>>),
    ArgumentBuffer {
        buffer: metal::Buffer,
        offset: NSUInteger,
        encoder: metal::ArgumentEncoder,
        stage_flags: pso::ShaderStageFlags,
    }
}
unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {}

#[derive(Debug)]
pub struct DescriptorSetInner {
    pub(crate) layout: Vec<pso::DescriptorSetLayoutBinding>, // TODO: maybe don't clone?
    pub(crate) bindings: HashMap<usize, DescriptorSetBinding>,
}
unsafe impl Send for DescriptorSetInner {}

#[derive(Debug)]
pub enum DescriptorSetBinding {
    Sampler(Vec<Option<metal::SamplerState>>),
    SampledImage(Vec<Option<(metal::Texture, image::ImageLayout)>>),
    //StorageImage(Vec<(metal::Texture, image::ImageLayout)>),
    //UniformTexelBuffer,
    //StorageTexelBuffer,
    ConstantBuffer(Vec<Option<(metal::Buffer, u64)>>),
    //StorageBuffer,
    //InputAttachment(Vec<(metal::Texture, image::ImageLayout)>),
}

#[derive(Debug)]
pub enum Memory {
    Emulated { memory_type: usize, size: u64 },
    Native(metal::Heap),
}

#[derive(Debug)]
pub struct UnboundBuffer {
    pub(crate) size: u64,
}

unsafe impl Send for UnboundBuffer {}
unsafe impl Sync for UnboundBuffer {}

#[derive(Debug)]
pub struct UnboundImage {
    pub desc: metal::TextureDescriptor,
    pub bytes_per_block: u8,
    // Dimension of a texel block (compressed formats).
    pub block_dim: (u8, u8),
}
unsafe impl Send for UnboundImage {}
unsafe impl Sync for UnboundImage {}

#[derive(Debug)]
pub struct Fence(pub Arc<Mutex<bool>>);


pub unsafe fn objc_err_description(object: *mut objc::runtime::Object) -> String {
    let description: *mut objc::runtime::Object = msg_send![object, localizedDescription];
    let utf16_len: NSUInteger = msg_send![description, length];
    let utf8_bytes: NSUInteger = msg_send![description, lengthOfBytesUsingEncoding: 4 as NSUInteger];
    let mut bytes = Vec::with_capacity(utf8_bytes as usize);
    bytes.set_len(utf8_bytes as usize);
    let success: objc::runtime::BOOL = msg_send![description,
        getBytes: bytes.as_mut_ptr()
        maxLength: utf8_bytes
        usedLength: ptr::null_mut::<NSUInteger>()
        encoding: 4 as NSUInteger
        options: 0 as c_int
        range: NSRange  { location: 0, length: utf16_len }
        remainingRange: ptr::null_mut::<NSRange>()
    ];
    if success == objc::runtime::YES {
        String::from_utf8_unchecked(bytes)
    } else {
        panic!("failed to get object description")
    }
}

extern "C" {
    #[allow(dead_code)]
    pub fn dispatch_semaphore_wait(
        semaphore: *mut c_void,
        timeout: u64,
    ) -> c_long;

    pub fn dispatch_semaphore_signal(
        semaphore: *mut c_void,
    ) -> c_long;

    pub fn dispatch_semaphore_create(
        value: c_long,
    ) -> *mut c_void;

    pub fn dispatch_release(
        object: *mut c_void,
    );
}
