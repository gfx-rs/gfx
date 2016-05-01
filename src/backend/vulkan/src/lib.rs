// Copyright 2016 The Gfx-rs Developers.
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

extern crate shared_library;
extern crate gfx_core;

use std::{fmt, mem, ptr};
use std::ffi::CString;
use std::path::Path;
use shared_library::dynamic_library::DynamicLibrary;

mod vk {
    #![allow(dead_code)]
    #![allow(non_upper_case_globals)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    include!(concat!(env!("OUT_DIR"), "/vk_bindings.rs"));
}


/// Information that can be given to the Vulkan driver so that it can identify your application.
pub struct ApplicationInfo<'a> {
    /// Name of the application.
    pub application_name: &'a str,
    /// An opaque number that contains the version number of the application.
    pub application_version: u32,
    /// Name of the engine used to power the application.
    pub engine_name: &'a str,
    /// An opaque number that contains the version number of the engine.
    pub engine_version: u32,
}


struct PhysicalDeviceInfo {
    device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
    queue_families: Vec<vk::QueueFamilyProperties>,
    memory: vk::PhysicalDeviceMemoryProperties,
    //available_features: Features,
}

pub struct Backend {
    dynamic_lib: DynamicLibrary,
    library: vk::Static,
    instance: vk::Instance,
    inst_pointers: vk::InstancePointers,
    functions: vk::EntryPoints,
    devices: Vec<PhysicalDeviceInfo>,
}

pub fn create(app_info: Option<ApplicationInfo>) -> Backend {
    let c_app_name: CString;
    let c_engine_name: CString;
    let vk_info: vk::ApplicationInfo;

    let dynamic_lib = DynamicLibrary::open(Some(Path::new("libvulkan.so"))).unwrap();
    let lib = vk::Static::load(|name| unsafe {
        let name = name.to_str().unwrap();
        dynamic_lib.symbol(name).unwrap()
    });
    let entry_points = vk::EntryPoints::load(|name| unsafe {
        mem::transmute(lib.GetInstanceProcAddr(0, name.as_ptr()))
    });

    let info_ptr = if let Some(info) = app_info {
        c_app_name = CString::new(info.application_name).unwrap();
        c_engine_name = CString::new(info.engine_name).unwrap();
        vk_info = vk::ApplicationInfo {
            sType: vk::STRUCTURE_TYPE_APPLICATION_INFO,
            pNext: ptr::null(),
            pApplicationName: c_app_name.as_ptr(),
            applicationVersion: info.application_version,
            pEngineName: c_engine_name.as_ptr(),
            engineVersion: info.engine_version,
            apiVersion: 0x400000, //TODO
        };
        &vk_info as *const _
    }else {
        ptr::null()
    };

    let create_info = vk::InstanceCreateInfo {
        sType: vk::STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        pNext: ptr::null(),
        flags: 0,
        pApplicationInfo: info_ptr,
        enabledLayerCount: 0, //TODO
        ppEnabledLayerNames: ptr::null(), //TODO
        enabledExtensionCount: 0, //TODO
        ppEnabledExtensionNames: ptr::null(), //TODO
    };

    let instance = unsafe {
        let mut ptr = mem::uninitialized();
        let status = entry_points.CreateInstance(&create_info, ptr::null(), &mut ptr);
        if status != vk::SUCCESS {
            panic!("vkCreateInstance: {:?}", Error(status));
        }
        ptr
    };

    let inst_ppinters = vk::InstancePointers::load(|name| unsafe {
        mem::transmute(lib.GetInstanceProcAddr(instance, name.as_ptr()))
    });

    Backend {
        dynamic_lib: dynamic_lib,
        library: lib,
        instance: instance,
        inst_pointers: inst_ppinters,
        functions: entry_points,
        devices: Vec::new(),
    }
}


#[derive(Clone, PartialEq, Eq)]
pub struct Error(vk::Result);

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self.0 {
            vk::SUCCESS => "success",
            vk::NOT_READY => "not ready",
            vk::TIMEOUT => "timeout",
            vk::EVENT_SET => "event_set",
            vk::EVENT_RESET => "event_reset",
            vk::INCOMPLETE => "incomplete",
            vk::ERROR_OUT_OF_HOST_MEMORY => "out of host memory",
            vk::ERROR_OUT_OF_DEVICE_MEMORY => "out of device memory",
            vk::ERROR_INITIALIZATION_FAILED => "initialization failed",
            vk::ERROR_DEVICE_LOST => "device lost",
            vk::ERROR_MEMORY_MAP_FAILED => "memory map failed",
            vk::ERROR_LAYER_NOT_PRESENT => "layer not present",
            vk::ERROR_EXTENSION_NOT_PRESENT => "extension not present",
            vk::ERROR_FEATURE_NOT_PRESENT => "feature not present",
            vk::ERROR_INCOMPATIBLE_DRIVER => "incompatible driver",
            vk::ERROR_TOO_MANY_OBJECTS => "too many objects",
            vk::ERROR_FORMAT_NOT_SUPPORTED => "format not supported",
            vk::ERROR_SURFACE_LOST_KHR => "surface lost (KHR)",
            vk::ERROR_NATIVE_WINDOW_IN_USE_KHR => "native window in use (KHR)",
            vk::SUBOPTIMAL_KHR => "suboptimal (KHR)",
            vk::ERROR_OUT_OF_DATE_KHR => "out of date (KHR)",
            vk::ERROR_INCOMPATIBLE_DISPLAY_KHR => "incompatible display (KHR)",
            vk::ERROR_VALIDATION_FAILED_EXT => "validation failed (EXT)",
            _ => "unknown",
        })
    }
}