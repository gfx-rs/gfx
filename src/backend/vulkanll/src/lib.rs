// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use]
extern crate log;

extern crate ash;
extern crate gfx_corell as core;
#[macro_use]
extern crate lazy_static;
extern crate winit;

#[cfg(target_os = "windows")]
extern crate kernel32;

use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0, V1_0};
use ash::vk;
use ash::{Entry, LoadingError};
use core::{format, memory, QueueSubmit, FrameSync};
use core::command::Submit;
use std::ffi::{CStr, CString};
use std::mem;
use std::ptr;
use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::VecDeque;

mod command;
mod data;
mod factory;
mod native;
mod pool;
mod state;

pub use command::{RenderPassInlineEncoder, RenderPassSecondaryEncoder};
pub use pool::{GeneralCommandPool, GraphicsCommandPool,
    ComputeCommandPool, TransferCommandPool, SubpassCommandPool};

lazy_static! {
    static ref VK_ENTRY: Result<Entry<V1_0>, LoadingError> = Entry::new();
}

pub struct QueueFamily {
    instance: Arc<InstanceInner>,
    device: vk::PhysicalDevice,
    family_index: u32,
    queue_type: vk::QueueFlags,
    queue_count: u32,
}

impl core::QueueFamily for QueueFamily {
    type Surface = Surface;

    fn supports_present(&self, surface: &Self::Surface) -> bool {
        unsafe {
            let mut support = mem::uninitialized();
            surface.inner.loader.get_physical_device_surface_support_khr(
                self.device,
                self.family_index,
                surface.inner.handle,
                &mut support);
            support == vk::VK_TRUE
        }
    }

    fn num_queues(&self) -> u32 {
        self.queue_count
    }
}

pub struct Adapter {
    handle: vk::PhysicalDevice,
    queue_families: Vec<QueueFamily>,
    info: core::AdapterInfo,
    instance: Arc<InstanceInner>,
}

impl core::Adapter for Adapter {
    type CommandQueue = CommandQueue;
    type Resources = Resources;
    type Factory = Factory;
    type QueueFamily = QueueFamily;

    fn open<'a, I>(&self, queue_descs: I) -> core::Device<Resources, Factory, CommandQueue>
        where I: ExactSizeIterator<Item=(&'a QueueFamily, u32)>
    {
        let mut queue_priorities = Vec::with_capacity(queue_descs.len());

        let queue_infos = queue_descs.map(|(family, queue_count)| {
                queue_priorities.push(vec![0.0f32; queue_count as usize]);

                vk::DeviceQueueCreateInfo {
                    s_type: vk::StructureType::DeviceQueueCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::DeviceQueueCreateFlags::empty(),
                    queue_family_index: family.family_index,
                    queue_count: queue_count,
                    p_queue_priorities: queue_priorities.last().unwrap().as_ptr(),
                }
            }).collect::<Vec<_>>();

        // Create device
        let device_extensions = &[vk::VK_KHR_SWAPCHAIN_EXTENSION_NAME,];

        let device_raw = {
            let cstrings = device_extensions.iter()
                                    .map(|&s| CString::new(s).unwrap())
                                    .collect::<Vec<_>>();

            let str_pointers = cstrings.iter()
                                    .map(|s| s.as_ptr())
                                    .collect::<Vec<_>>();

            let features = unsafe { mem::zeroed() };
            let info = vk::DeviceCreateInfo {
                s_type: vk::StructureType::DeviceCreateInfo,
                p_next: ptr::null(),
                flags: vk::DeviceCreateFlags::empty(),
                queue_create_info_count: queue_infos.len() as u32,
                p_queue_create_infos: queue_infos.as_ptr(),
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: str_pointers.len() as u32,
                pp_enabled_extension_names: str_pointers.as_ptr(),
                p_enabled_features: &features,
            };

            unsafe {
                self.instance.0.create_device(self.handle, &info, None)
                    .expect("Error on device creation")
            }
        };

        let factory = Factory {
            inner: Arc::new(DeviceInner(device_raw)),
        };

        let mem_properties = self.instance.0.get_physical_device_memory_properties(self.handle);
        let memory_heaps = mem_properties.memory_heaps[..mem_properties.memory_heap_count as usize].iter()
                                .map(|mem| mem.size).collect::<Vec<_>>();
        let heap_types = mem_properties.memory_types[..mem_properties.memory_type_count as usize].iter().enumerate().map(|(i, mem)| {
            let mut type_flags = memory::HeapProperties::empty();

            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_DEVICE_LOCAL_BIT) {
                type_flags |= memory::DEVICE_LOCAL;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_COHERENT_BIT) {
                type_flags |= memory::COHERENT;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_CACHED_BIT) {
                type_flags |= memory::CPU_CACHED;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT) {
                type_flags |= memory::CPU_VISIBLE;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT) {
                type_flags |= memory::LAZILY_ALLOCATED;
            }
            
            core::HeapType {
                id: i,
                properties: type_flags,
                heap_index: mem.heap_index as usize,
            }
        }).collect::<Vec<_>>();

        // Create associated command queues for each queue type
        let queues = queue_infos.iter().flat_map(|info| {
            (0..info.queue_count).map(|id| {
                let queue = unsafe {
                    factory.inner.0.get_device_queue(info.queue_family_index, id)
                };
                // TODO:
                unsafe {
                    core::GeneralQueue::new(CommandQueue {
                        inner: CommandQueueInner(Rc::new(RefCell::new(queue))),
                        device: factory.inner.clone(),
                        family_index: info.queue_family_index,
                    })
                }
            }).collect::<Vec<_>>()
        }).collect();

        core::Device {
            factory: factory,
            general_queues: queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types: heap_types,
            memory_heaps: memory_heaps,

            _marker: std::marker::PhantomData,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.info
    }

    fn get_queue_families(&self) -> std::slice::Iter<Self::QueueFamily> {
        self.queue_families.iter()
    }
}

#[doc(hidden)]
pub struct DeviceInner(ash::Device<V1_0>);
impl Drop for DeviceInner {
    fn drop(&mut self) {
        unsafe { self.0.destroy_device(None); }
    }
}

pub struct Factory {
    inner: Arc<DeviceInner>,
}

// # Synchronization
//  vk::Queue needs to be externally synchronized on vkQueueSubmit.
//  Current approach is based on Rc and RefCell not implementing Sync and submit requires mutable access.
//  So we can clone the inner command queue for the swapchain which also needs it for present.
//  We internally build some sort of dependency graph using reference counting to unsure everything lives long enough.
#[derive(Clone)]
struct CommandQueueInner(Rc<RefCell<vk::Queue>>);

pub struct CommandQueue {
    inner: CommandQueueInner,
    device: Arc<DeviceInner>,
    family_index: u32,
}

impl core::CommandQueue for CommandQueue {
    type R = Resources;
    type SubmitInfo = command::SubmitInfo;
    type GeneralCommandBuffer = native::GeneralCommandBuffer;
    type GraphicsCommandBuffer = native::GraphicsCommandBuffer;
    type ComputeCommandBuffer = native::ComputeCommandBuffer;
    type TransferCommandBuffer = native::TransferCommandBuffer;
    type SubpassCommandBuffer = native::SubpassCommandBuffer;

    unsafe fn submit<C>(&mut self, submit_infos: &[QueueSubmit<C, Resources>], fence: Option<&mut native::Fence>)
        where C: core::CommandBuffer<SubmitInfo = command::SubmitInfo>
    {
        let mut command_buffers = Vec::with_capacity(submit_infos.len());
        let mut wait_semaphores = Vec::with_capacity(submit_infos.len());
        let mut wait_stages = Vec::with_capacity(submit_infos.len());
        let mut signal_semaphores = Vec::with_capacity(submit_infos.len());

        let submits = submit_infos.iter().map(|submit| {
            let cmd_buffers = submit.cmd_buffers
                                   .iter().map(|submit| submit.get_info().command_buffer)
                                   .collect::<Vec<_>>();
            let waits = submit.wait_semaphores.iter().map(|&(ref semaphore, _)| semaphore.0).collect::<Vec<_>>();
            let stages = submit.wait_semaphores.iter().map(|&(_, stage)| data::map_pipeline_stage(stage)).collect::<Vec<_>>();
            let signals = submit.signal_semaphores.iter().map(|semaphore| semaphore.0).collect::<Vec<_>>();

            command_buffers.push(cmd_buffers);
            wait_semaphores.push(waits);
            wait_stages.push(stages);
            signal_semaphores.push(signals);

            let wait_semaphores = wait_semaphores.last().unwrap();
            let wait_stages = wait_stages.last().unwrap();

            vk::SubmitInfo {
                s_type: vk::StructureType::SubmitInfo,
                p_next: ptr::null(),
                wait_semaphore_count: wait_semaphores.len() as u32,
                p_wait_semaphores: wait_semaphores.as_ptr(),
                // If count is zero, AMD driver crashes if nullptr is not set for stage masks
                p_wait_dst_stage_mask: if wait_stages.is_empty() { ptr::null() } else { wait_stages.as_ptr() }, 
                command_buffer_count: command_buffers.last().unwrap().len() as u32,
                p_command_buffers: command_buffers.last().unwrap().as_ptr(),
                signal_semaphore_count: signal_semaphores.last().unwrap().len() as u32,
                p_signal_semaphores: signal_semaphores.last().unwrap().as_ptr(),
            }
        }).collect::<Vec<_>>();

        let fence = fence.map(|fence| fence.0).unwrap_or(vk::Fence::null());

        unsafe {
            self.device.0.queue_submit(
                *self.inner.0.borrow(),
                &submits,
                fence,
            );
        }
    }

    fn wait_idle(&mut self) {
        unsafe {
            self.device.0.queue_wait_idle(*self.inner.0.borrow());
        }
    }
}

struct SurfaceInner {
    handle: vk::SurfaceKHR,
    instance: Arc<InstanceInner>,
    loader: vk::SurfaceFn,
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface_khr(self.instance.0.handle(), self.handle, ptr::null()); }
    }
}

pub struct Surface {
    // Vk (EXT) specs [29.2.7 Platform-Independent Information]
    // For vkDestroySurfaceKHR: Host access to surface must be externally synchronized
    inner: Arc<SurfaceInner>,
    width: u32,
    height: u32,
}

impl Surface {
    fn from_raw(instance: &Instance, surface: vk::SurfaceKHR, (width, height): (u32, u32)) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let loader = vk::SurfaceFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        instance.inner.0.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load surface functions");

        let inner = Arc::new(SurfaceInner {
            handle: surface,
            instance: instance.inner.clone(),
            loader: loader,
        });

        Surface {
            inner: inner,
            width: width,
            height: height,
        }
    }
}

impl core::Surface for Surface {
    type Queue = CommandQueue;
    type SwapChain = SwapChain;

    fn build_swapchain<T: core::format::RenderFormat>(&self,
                    present_queue: &CommandQueue) -> SwapChain {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let loader = vk::SwapchainFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        self.inner.instance.0.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load swapchain functions");

        // TODO: check for better ones if available
        let present_mode = vk::PresentModeKHR::Fifo; // required to be supported

        let format = <T as format::Formatted>::get_format();

        let info = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SwapchainCreateInfoKhr,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: self.inner.handle,
            min_image_count: 2, // TODO: let the user specify the value
            image_format: data::map_format(format.0, format.1).unwrap(),
            image_color_space: vk::ColorSpaceKHR::SrgbNonlinear,
            image_extent: vk::Extent2D {
                width: self.width,
                height: self.height
            },
            image_array_layers: 1,
            image_usage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            image_sharing_mode: vk::SharingMode::Exclusive,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
            composite_alpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
            present_mode: present_mode,
            clipped: 1,
            old_swapchain: vk::SwapchainKHR::null(), 
        };

        let swapchain = unsafe {
            let mut swapchain = mem::uninitialized();
            assert_eq!(vk::Result::Success, unsafe {
                loader.create_swapchain_khr(
                    present_queue.device.0.handle(),
                    &info,
                    ptr::null(),
                    &mut swapchain)
            });
            swapchain
        };

        let swapchain_images = unsafe {
            // TODO: error handling
            let mut count = 0;
            loader.get_swapchain_images_khr(
                present_queue.device.0.handle(),
                swapchain,
                &mut count,
                ptr::null_mut());

            let mut v = Vec::with_capacity(count as vk::size_t);
            loader.get_swapchain_images_khr(
                present_queue.device.0.handle(),
                swapchain,
                &mut count,
                v.as_mut_ptr());

            v.set_len(count as vk::size_t);
            v.into_iter().map(|image| native::Image(image))
                    .collect::<Vec<_>>()
        };

        // TODO: set initial resource states to Present

        SwapChain {
            inner: swapchain,
            present_queue: present_queue.inner.clone(),
            device: present_queue.device.clone(),
            swapchain_fn: loader,
            images: swapchain_images,
            frame_queue: VecDeque::new(),
        }
    }
}

pub struct SwapChain {
    inner: vk::SwapchainKHR,
    device: Arc<DeviceInner>,
    present_queue: CommandQueueInner,
    swapchain_fn: vk::SwapchainFn,
    images: Vec<native::Image>,

    // Queued up frames for presentation
    frame_queue: VecDeque<usize>,
}

impl core::SwapChain for SwapChain {
    type Image = native::Image;
    type R = Resources;

    fn get_images(&mut self) -> &[native::Image] {
       &self.images
    }

    fn acquire_frame(&mut self, sync: FrameSync<Resources>) -> core::Frame {
        let (semaphore, fence) = match sync {
            FrameSync::Semaphore(semaphore) => (semaphore.0, vk::Fence::null()),
            FrameSync::Fence(fence) => (vk::Semaphore::null(), fence.0),
        };

        // TODO: error handling
        let index = unsafe {
            let mut index = mem::uninitialized();
            self.swapchain_fn.acquire_next_image_khr(
                    self.device.0.handle(),
                    self.inner,
                    std::u64::MAX, // will block if no image is available
                    semaphore,
                    fence,
                    &mut index);
            index
        };

        self.frame_queue.push_back(index as usize);
        unsafe { core::Frame::new(index as usize) }
    }

    fn present(&mut self) {
        let frame = self.frame_queue.pop_front().expect("No frame currently queued up. Need to acquire a frame first.");

        // TODO: ensure correct image layout (present)
        let info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PresentInfoKhr,
            p_next: ptr::null(),
            wait_semaphore_count: 0,
            p_wait_semaphores: ptr::null(),
            swapchain_count: 1,
            p_swapchains: &self.inner,
            p_image_indices: &(frame as u32),
            p_results: ptr::null_mut(),
        };
        unsafe {
            self.swapchain_fn.queue_present_khr(*self.present_queue.0.borrow(), &info);
        }
        // TODO: handle result and return code
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        unsafe {
            self.swapchain_fn.destroy_swapchain_khr(
                self.device.0.handle(),
                self.inner,
                std::ptr::null());
        }
    }
}

struct InstanceInner(pub ash::Instance<V1_0>);
impl Drop for InstanceInner {
    fn drop(&mut self) {
        unsafe { self.0.destroy_instance(None); }
    }
}

const SURFACE_EXTENSIONS: &'static [&'static str] = &[
    vk::VK_KHR_SURFACE_EXTENSION_NAME,

    // Platform-specific WSI extensions
    vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_XCB_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_MIR_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_ANDROID_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_WIN32_SURFACE_EXTENSION_NAME,
];

pub struct Instance {
    // Vk specs [2.5 Threading Behavior]
    // Externally Synchronized Parameters: The `instance` parameter in `vkDestroyInstance`
    // `Arc` ensures that we only call drop once
    inner: Arc<InstanceInner>,

    /// Supported surface extensions of this instance.
    surface_extensions: Vec<&'static str>,
}

impl core::Instance for Instance {
    type Adapter = Adapter;
    type Surface = Surface;
    type Window = winit::Window;

    fn create() -> Instance {
        // TODO: return errors instead of panic
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::ApplicationInfo,
            p_next: ptr::null(),
            p_application_name: "vulkan_ll".as_ptr() as *const _, // TODO:
            application_version: 0,
            p_engine_name: "gfx-rs".as_ptr() as *const _,
            engine_version: 0, //TODO
            api_version: 0, //TODO
        };

        let instance_extensions = entry.enumerate_instance_extension_properties()
                                       .expect("Unable to enumerate instance extensions");

        // Check our surface extensions against the available extensions
        let surface_extensions = SURFACE_EXTENSIONS.iter().filter_map(|ext| {
            instance_extensions.iter().find(|inst_ext| {
                unsafe { CStr::from_ptr(inst_ext.extension_name.as_ptr()) == CStr::from_ptr(ext.as_ptr() as *const i8) }
            }).and_then(|_| Some(*ext))
        }).collect::<Vec<&str>>();

        let instance = {
            let cstrings = surface_extensions.iter()
                                    .map(|&s| CString::new(s).unwrap())
                                    .collect::<Vec<_>>();

            let str_pointers = cstrings.iter()
                                    .map(|s| s.as_ptr())
                                    .collect::<Vec<_>>();

            let create_info = vk::InstanceCreateInfo {
                s_type: vk::StructureType::InstanceCreateInfo,
                p_next: ptr::null(),
                flags: vk::InstanceCreateFlags::empty(),
                p_application_info: &app_info,
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: str_pointers.len() as u32,
                pp_enabled_extension_names: str_pointers.as_ptr(),
            };

            entry.create_instance(&create_info, None).expect("Unable to create vulkan instance")
        };

        Instance {
            inner: Arc::new(InstanceInner(instance)),
            surface_extensions: surface_extensions,
        }
    }

    fn enumerate_adapters(&self) -> Vec<Adapter> {
        self.inner.0.enumerate_physical_devices()
            .expect("Unable to enumerate adapter")
            .iter()
            .map(|&device| {
                let properties = self.inner.0.get_physical_device_properties(device);
                let name = unsafe {
                    CStr::from_ptr(properties.device_name.as_ptr())
                            .to_str()
                            .expect("Invalid UTF-8 string")
                            .to_owned()
                };

                let info = core::AdapterInfo {
                    name: name,
                    vendor: properties.vendor_id as usize,
                    device: properties.device_id as usize,
                    software_rendering: properties.device_type == vk::PhysicalDeviceType::Cpu,
                };

                let queue_families = self.inner.0.get_physical_device_queue_family_properties(device)
                                                 .iter()
                                                 .enumerate()
                                                 .map(|(i, queue_family)| {
                                                    QueueFamily {
                                                        instance: self.inner.clone(),
                                                        device: device,
                                                        family_index: i as u32,
                                                        queue_type: queue_family.queue_flags,
                                                        queue_count: queue_family.queue_count,
                                                    }
                                                 }).collect();

                Adapter {
                    handle: device,
                    queue_families: queue_families,
                    info: info,
                    instance: self.inner.clone(),
                }
            })
            .collect()
    }

    #[cfg(not(target_os = "windows"))]
    fn create_surface(&self, window: &winit::Window) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let surface = self.surface_extensions.iter().map(|&extension| {
            match extension {
                vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME => {
                    use winit::os::unix::WindowExt;
                    let xlib_loader = if let Ok(loader) = ash::extensions::XlibSurface::new(entry, &self.inner.0) {
                        loader
                    } else {
                        return None;
                    };

                    unsafe {
                        let info = vk::XlibSurfaceCreateInfoKHR {
                            s_type: vk::StructureType::XlibSurfaceCreateInfoKhr,
                            p_next: ptr::null(),
                            flags: vk::XlibSurfaceCreateFlagsKHR::empty(),
                            window: window.get_xlib_window().unwrap() as *const _,
                            dpy: window.get_xlib_display().unwrap() as *mut _,
                        };

                        xlib_loader.create_xlib_surface_khr(&info, None).ok()
                    }
                },
                // TODO: other platforms
                _ => None,
            }
        }).find(|x| x.is_some())
          .expect("Unable to find a surface implementation.")
          .unwrap();

        Surface::from_raw(self, surface, window.get_inner_size_pixels().unwrap())
    }

    #[cfg(target_os = "windows")]
    fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::windows::WindowExt;
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let win32_loader = ash::extensions::Win32Surface::new(entry, &self.inner.0)
                        .expect("Unable to load win32 surface functions");

        let surface = unsafe {
            let info = vk::Win32SurfaceCreateInfoKHR {
                s_type: vk::StructureType::Win32SurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::Win32SurfaceCreateFlagsKHR::empty(),
                hinstance: unsafe { kernel32::GetModuleHandleW(ptr::null()) } as *mut _,
                hwnd: window.get_hwnd() as *mut _,
            };

            win32_loader.create_win32_surface_khr(&info, None)
                .expect("Error on surface creation")
        };

        Surface::from_raw(self, surface, window.get_inner_size_pixels().unwrap())
    }
}

pub enum Backend { }
impl core::Backend for Backend {
    type CommandQueue = CommandQueue;
    type Factory = Factory;
    type Instance = Instance;
    type Adapter = Adapter;
    type Resources = Resources;
    type Surface = Surface;
    type SwapChain = SwapChain;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources { }
impl core::Resources for Resources {
    type ShaderLib = native::ShaderLib;
    type RenderPass = native::RenderPass;
    type PipelineLayout = native::PipelineLayout;
    type FrameBuffer = native::FrameBuffer;
    type GraphicsPipeline = native::GraphicsPipeline;
    type ComputePipeline = native::ComputePipeline;
    type UnboundBuffer = factory::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = factory::UnboundImage;
    type Image = native::Image;
    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;
    type Sampler = native::Sampler;
    type Semaphore = native::Semaphore;
    type Fence = native::Fence;
    type Heap = native::Heap;
    type Mapping = factory::Mapping;
    type DescriptorHeap = native::DescriptorHeap;
    type DescriptorSetPool = native::DescriptorSetPool;
    type DescriptorSet = native::DescriptorSet;
    type DescriptorSetLayout = native::DescriptorSetLayout;
}
