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

extern crate gfx_core;

use std::ffi::CString;
use std::ptr;

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
    instance: vk::Instance,
    pointers: vk::InstancePointers,
    devices: Vec<PhysicalDeviceInfo>,
}

pub fn create(app_info: Option<&ApplicationInfo>) -> Backend {
    let mut c_app_name: CString;
    let mut c_engine_name: CString;
    let mut vk_info: vk::ApplicationInfo;

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
            apiVersion: 0x1000, //TODO
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

    Backend {

    }
}
