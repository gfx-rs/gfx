pub(crate) fn map_device_features(
    features: Features,
    imageless_framebuffers: bool,
) -> crate::DeviceCreationFeatures {
    crate::DeviceCreationFeatures {
        // vk::PhysicalDeviceFeatures is a struct composed of Bool32's while
        // Features is a bitfield so we need to map everything manually
        core: vk::PhysicalDeviceFeatures::builder()
            .robust_buffer_access(features.contains(Features::ROBUST_BUFFER_ACCESS))
            .full_draw_index_uint32(features.contains(Features::FULL_DRAW_INDEX_U32))
            .image_cube_array(features.contains(Features::IMAGE_CUBE_ARRAY))
            .independent_blend(features.contains(Features::INDEPENDENT_BLENDING))
            .geometry_shader(features.contains(Features::GEOMETRY_SHADER))
            .tessellation_shader(features.contains(Features::TESSELLATION_SHADER))
            .sample_rate_shading(features.contains(Features::SAMPLE_RATE_SHADING))
            .dual_src_blend(features.contains(Features::DUAL_SRC_BLENDING))
            .logic_op(features.contains(Features::LOGIC_OP))
            .multi_draw_indirect(features.contains(Features::MULTI_DRAW_INDIRECT))
            .draw_indirect_first_instance(features.contains(Features::DRAW_INDIRECT_FIRST_INSTANCE))
            .depth_clamp(features.contains(Features::DEPTH_CLAMP))
            .depth_bias_clamp(features.contains(Features::DEPTH_BIAS_CLAMP))
            .fill_mode_non_solid(features.contains(Features::NON_FILL_POLYGON_MODE))
            .depth_bounds(features.contains(Features::DEPTH_BOUNDS))
            .wide_lines(features.contains(Features::LINE_WIDTH))
            .large_points(features.contains(Features::POINT_SIZE))
            .alpha_to_one(features.contains(Features::ALPHA_TO_ONE))
            .multi_viewport(features.contains(Features::MULTI_VIEWPORTS))
            .sampler_anisotropy(features.contains(Features::SAMPLER_ANISOTROPY))
            .texture_compression_etc2(features.contains(Features::FORMAT_ETC2))
            .texture_compression_astc_ldr(features.contains(Features::FORMAT_ASTC_LDR))
            .texture_compression_bc(features.contains(Features::FORMAT_BC))
            .occlusion_query_precise(features.contains(Features::PRECISE_OCCLUSION_QUERY))
            .pipeline_statistics_query(features.contains(Features::PIPELINE_STATISTICS_QUERY))
            .vertex_pipeline_stores_and_atomics(
                features.contains(Features::VERTEX_STORES_AND_ATOMICS),
            )
            .fragment_stores_and_atomics(features.contains(Features::FRAGMENT_STORES_AND_ATOMICS))
            .shader_tessellation_and_geometry_point_size(
                features.contains(Features::SHADER_TESSELLATION_AND_GEOMETRY_POINT_SIZE),
            )
            .shader_image_gather_extended(features.contains(Features::SHADER_IMAGE_GATHER_EXTENDED))
            .shader_storage_image_extended_formats(
                features.contains(Features::SHADER_STORAGE_IMAGE_EXTENDED_FORMATS),
            )
            .shader_storage_image_multisample(
                features.contains(Features::SHADER_STORAGE_IMAGE_MULTISAMPLE),
            )
            .shader_storage_image_read_without_format(
                features.contains(Features::SHADER_STORAGE_IMAGE_READ_WITHOUT_FORMAT),
            )
            .shader_storage_image_write_without_format(
                features.contains(Features::SHADER_STORAGE_IMAGE_WRITE_WITHOUT_FORMAT),
            )
            .shader_uniform_buffer_array_dynamic_indexing(
                features.contains(Features::SHADER_UNIFORM_BUFFER_ARRAY_DYNAMIC_INDEXING),
            )
            .shader_sampled_image_array_dynamic_indexing(
                features.contains(Features::SHADER_SAMPLED_IMAGE_ARRAY_DYNAMIC_INDEXING),
            )
            .shader_storage_buffer_array_dynamic_indexing(
                features.contains(Features::SHADER_STORAGE_BUFFER_ARRAY_DYNAMIC_INDEXING),
            )
            .shader_storage_image_array_dynamic_indexing(
                features.contains(Features::SHADER_STORAGE_IMAGE_ARRAY_DYNAMIC_INDEXING),
            )
            .shader_clip_distance(features.contains(Features::SHADER_CLIP_DISTANCE))
            .shader_cull_distance(features.contains(Features::SHADER_CULL_DISTANCE))
            .shader_float64(features.contains(Features::SHADER_FLOAT64))
            .shader_int64(features.contains(Features::SHADER_INT64))
            .shader_int16(features.contains(Features::SHADER_INT16))
            .shader_resource_residency(features.contains(Features::SHADER_RESOURCE_RESIDENCY))
            .shader_resource_min_lod(features.contains(Features::SHADER_RESOURCE_MIN_LOD))
            .sparse_binding(features.contains(Features::SPARSE_BINDING))
            .sparse_residency_buffer(features.contains(Features::SPARSE_RESIDENCY_BUFFER))
            .sparse_residency_image2_d(features.contains(Features::SPARSE_RESIDENCY_IMAGE_2D))
            .sparse_residency_image3_d(features.contains(Features::SPARSE_RESIDENCY_IMAGE_3D))
            .sparse_residency2_samples(features.contains(Features::SPARSE_RESIDENCY_2_SAMPLES))
            .sparse_residency4_samples(features.contains(Features::SPARSE_RESIDENCY_4_SAMPLES))
            .sparse_residency8_samples(features.contains(Features::SPARSE_RESIDENCY_8_SAMPLES))
            .sparse_residency16_samples(features.contains(Features::SPARSE_RESIDENCY_16_SAMPLES))
            .sparse_residency_aliased(features.contains(Features::SPARSE_RESIDENCY_ALIASED))
            .variable_multisample_rate(features.contains(Features::VARIABLE_MULTISAMPLE_RATE))
            .inherited_queries(features.contains(Features::INHERITED_QUERIES))
            .build(),
        descriptor_indexing: if features.intersects(Features::DESCRIPTOR_INDEXING_MASK) {
            Some(
                vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::builder()
                    .shader_sampled_image_array_non_uniform_indexing(
                        features.contains(Features::SAMPLED_TEXTURE_DESCRIPTOR_INDEXING),
                    )
                    .shader_storage_image_array_non_uniform_indexing(
                        features.contains(Features::STORAGE_TEXTURE_DESCRIPTOR_INDEXING),
                    )
                    .runtime_descriptor_array(features.contains(Features::UNSIZED_DESCRIPTOR_ARRAY))
                    .build(),
            )
        } else {
            None
        },
        mesh_shaders: if features.intersects(Features::MESH_SHADER_MASK) {
            Some(
                vk::PhysicalDeviceMeshShaderFeaturesNV::builder()
                    .task_shader(features.contains(Features::TASK_SHADER))
                    .mesh_shader(features.contains(Features::MESH_SHADER))
                    .build(),
            )
        } else {
            None
        },
        imageless_framebuffers: if imageless_framebuffers {
            Some(
                vk::PhysicalDeviceImagelessFramebufferFeaturesKHR::builder()
                    .imageless_framebuffer(imageless_framebuffers)
                    .build(),
            )
        } else {
            None
        },
    }
}

pub struct PhysicalDevice {
    api_version: Version,
    instance: Arc<RawInstance>,
    handle: vk::PhysicalDevice,
    extensions: Vec<vk::ExtensionProperties>,
    properties: vk::PhysicalDeviceProperties,
    known_memory_flags: vk::MemoryPropertyFlags,
}

impl PhysicalDevice {
    fn supports_extension(&self, extension: &CStr) -> bool {
        self.extensions
            .iter()
            .any(|ep| unsafe { CStr::from_ptr(ep.extension_name.as_ptr()) } == extension)
    }
}

impl fmt::Debug for PhysicalDevice {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("PhysicalDevice")
    }
}

pub struct DeviceCreationFeatures {
    core: vk::PhysicalDeviceFeatures,
    descriptor_indexing: Option<vk::PhysicalDeviceDescriptorIndexingFeaturesEXT>,
    mesh_shaders: Option<vk::PhysicalDeviceMeshShaderFeaturesNV>,
    imageless_framebuffers: Option<vk::PhysicalDeviceImagelessFramebufferFeaturesKHR>,
}

impl adapter::PhysicalDevice<Backend> for PhysicalDevice {
    unsafe fn open(
        &self,
        families: &[(&QueueFamily, &[queue::QueuePriority])],
        requested_features: Features,
    ) -> Result<adapter::Gpu<Backend>, DeviceCreationError> {
        let family_infos = families
            .iter()
            .map(|&(family, priorities)| {
                vk::DeviceQueueCreateInfo::builder()
                    .flags(vk::DeviceQueueCreateFlags::empty())
                    .queue_family_index(family.index)
                    .queue_priorities(priorities)
                    .build()
            })
            .collect::<Vec<_>>();

        if !self.features().contains(requested_features) {
            return Err(DeviceCreationError::MissingFeature);
        }

        let imageless_framebuffers = self.api_version >= Version::V1_2
            || self.supports_extension(vk::KhrImagelessFramebufferFn::name());

        let mut enabled_features =
            conv::map_device_features(requested_features, imageless_framebuffers);
        let enabled_extensions = {
            let mut requested_extensions: Vec<&'static CStr> = Vec::new();

            requested_extensions.push(extensions::khr::Swapchain::name());

            if self.api_version < Version::V1_1 {
                requested_extensions.push(vk::KhrMaintenance1Fn::name());
                requested_extensions.push(vk::KhrMaintenance2Fn::name());
            }

            if imageless_framebuffers && self.api_version < Version::V1_2 {
                requested_extensions.push(vk::KhrImagelessFramebufferFn::name());
                requested_extensions.push(vk::KhrImageFormatListFn::name()); // Required for `KhrImagelessFramebufferFn`
            }

            requested_extensions.push(vk::ExtSamplerFilterMinmaxFn::name());

            if requested_features.contains(Features::NDC_Y_UP) {
                // `VK_AMD_negative_viewport_height` is obsoleted by `VK_KHR_maintenance1` and must not be enabled alongside `VK_KHR_maintenance1` or a 1.1+ device.
                if self.api_version < Version::V1_1
                    && !self.supports_extension(vk::KhrMaintenance1Fn::name())
                {
                    requested_extensions.push(vk::AmdNegativeViewportHeightFn::name());
                }
            }

            if requested_features.intersects(Features::DESCRIPTOR_INDEXING_MASK)
                && self.api_version < Version::V1_2
            {
                requested_extensions.push(vk::ExtDescriptorIndexingFn::name());
                requested_extensions.push(vk::KhrMaintenance3Fn::name()); // Required for `ExtDescriptorIndexingFn`
            }

            if requested_features.intersects(Features::MESH_SHADER_MASK) {
                requested_extensions.push(MeshShader::name());
            }

            if requested_features.contains(Features::DRAW_INDIRECT_COUNT) {
                requested_extensions.push(DrawIndirectCount::name());
            }

            let (supported_extensions, unsupported_extensions) = requested_extensions
                .iter()
                .partition::<Vec<&CStr>, _>(|&&extension| self.supports_extension(extension));

            if !unsupported_extensions.is_empty() {
                warn!("Missing extensions: {:?}", unsupported_extensions);
            }

            debug!("Supported extensions: {:?}", supported_extensions);

            supported_extensions
        };

        let valid_ash_memory_types = {
            let mem_properties = self
                .instance
                .inner
                .get_physical_device_memory_properties(self.handle);
            mem_properties.memory_types[..mem_properties.memory_type_count as usize]
                .iter()
                .enumerate()
                .fold(0, |u, (i, mem)| {
                    if self.known_memory_flags.contains(mem.property_flags) {
                        u | (1 << i)
                    } else {
                        u
                    }
                })
        };

        // Create device
        let device_raw = {
            let str_pointers = enabled_extensions
                .iter()
                .map(|&s| {
                    // Safe because `enabled_extensions` entries have static lifetime.
                    s.as_ptr()
                })
                .collect::<Vec<_>>();

            let mut info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&family_infos)
                .enabled_extension_names(&str_pointers)
                .enabled_features(&enabled_features.core);
            if let Some(ref mut feature) = enabled_features.descriptor_indexing {
                info = info.push_next(feature);
            }
            if let Some(ref mut feature) = enabled_features.mesh_shaders {
                info = info.push_next(feature);
            }
            if let Some(ref mut feature) = enabled_features.imageless_framebuffers {
                info = info.push_next(feature);
            }

            match self.instance.inner.create_device(self.handle, &info, None) {
                Ok(device) => device,
                Err(e) => {
                    return Err(match e {
                        vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                            DeviceCreationError::OutOfMemory(OutOfMemory::Host)
                        }
                        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                            DeviceCreationError::OutOfMemory(OutOfMemory::Device)
                        }
                        vk::Result::ERROR_INITIALIZATION_FAILED => {
                            DeviceCreationError::InitializationFailed
                        }
                        vk::Result::ERROR_DEVICE_LOST => DeviceCreationError::DeviceLost,
                        vk::Result::ERROR_TOO_MANY_OBJECTS => DeviceCreationError::TooManyObjects,
                        _ => unreachable!(),
                    })
                }
            }
        };

        let swapchain_fn = Swapchain::new(&self.instance.inner, &device_raw);

        let mesh_fn = if requested_features.intersects(Features::MESH_SHADER_MASK) {
            Some(MeshShader::new(&self.instance.inner, &device_raw))
        } else {
            None
        };

        let indirect_count_fn = if requested_features.contains(Features::DRAW_INDIRECT_COUNT) {
            Some(DrawIndirectCount::new(&self.instance.inner, &device_raw))
        } else {
            None
        };

        #[cfg(feature = "naga")]
        let naga_options = {
            use naga::back::spv;
            let capabilities = [
                spv::Capability::Shader,
                spv::Capability::Matrix,
                spv::Capability::InputAttachment,
                spv::Capability::Sampled1D,
                spv::Capability::Image1D,
                spv::Capability::SampledBuffer,
                spv::Capability::ImageBuffer,
                spv::Capability::ImageQuery,
                spv::Capability::DerivativeControl,
                //TODO: fill out the rest
            ]
            .iter()
            .cloned()
            .collect();
            let mut flags = spv::WriterFlags::empty();
            if cfg!(debug_assertions) {
                flags |= spv::WriterFlags::DEBUG;
            }
            spv::Options {
                lang_version: (1, 0),
                flags,
                capabilities,
            }
        };

        let device = Device {
            shared: Arc::new(RawDevice {
                raw: device_raw,
                features: requested_features,
                instance: Arc::clone(&self.instance),
                extension_fns: DeviceExtensionFunctions {
                    mesh_shaders: mesh_fn,
                    draw_indirect_count: indirect_count_fn,
                },
                flip_y_requires_shift: self.api_version >= Version::V1_1
                    || self.supports_extension(vk::KhrMaintenance1Fn::name()),
                imageless_framebuffers,
                timestamp_period: self.properties.limits.timestamp_period,
            }),
            vendor_id: self.properties.vendor_id,
            valid_ash_memory_types,
            #[cfg(feature = "naga")]
            naga_options,
        };

        let device_arc = Arc::clone(&device.shared);
        let queue_groups = families
            .iter()
            .map(|&(family, ref priorities)| {
                let mut family_raw =
                    queue::QueueGroup::new(queue::QueueFamilyId(family.index as usize));
                for id in 0..priorities.len() {
                    let queue_raw = device_arc.raw.get_device_queue(family.index, id as _);
                    family_raw.add_queue(Queue {
                        raw: Arc::new(queue_raw),
                        device: device_arc.clone(),
                        swapchain_fn: swapchain_fn.clone(),
                    });
                }
                family_raw
            })
            .collect();

        Ok(adapter::Gpu {
            device,
            queue_groups,
        })
    }

    fn format_properties(&self, format: Option<format::Format>) -> format::Properties {
        let properties = unsafe {
            self.instance.inner.get_physical_device_format_properties(
                self.handle,
                format.map_or(vk::Format::UNDEFINED, conv::map_format),
            )
        };
        let supports_transfer_bits = self.supports_extension(vk::KhrMaintenance1Fn::name());

        format::Properties {
            linear_tiling: conv::map_image_features(
                properties.linear_tiling_features,
                supports_transfer_bits,
            ),
            optimal_tiling: conv::map_image_features(
                properties.optimal_tiling_features,
                supports_transfer_bits,
            ),
            buffer_features: conv::map_buffer_features(properties.buffer_features),
        }
    }

    fn image_format_properties(
        &self,
        format: format::Format,
        dimensions: u8,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        let format_properties = unsafe {
            self.instance
                .inner
                .get_physical_device_image_format_properties(
                    self.handle,
                    conv::map_format(format),
                    match dimensions {
                        1 => vk::ImageType::TYPE_1D,
                        2 => vk::ImageType::TYPE_2D,
                        3 => vk::ImageType::TYPE_3D,
                        _ => panic!("Unexpected image dimensionality: {}", dimensions),
                    },
                    conv::map_tiling(tiling),
                    conv::map_image_usage(usage),
                    conv::map_view_capabilities(view_caps),
                )
        };

        match format_properties {
            Ok(props) => Some(image::FormatProperties {
                max_extent: image::Extent {
                    width: props.max_extent.width,
                    height: props.max_extent.height,
                    depth: props.max_extent.depth,
                },
                max_levels: props.max_mip_levels as _,
                max_layers: props.max_array_layers as _,
                sample_count_mask: props.sample_counts.as_raw() as _,
                max_resource_size: props.max_resource_size as _,
            }),
            Err(vk::Result::ERROR_FORMAT_NOT_SUPPORTED) => None,
            Err(other) => {
                error!("Unexpected error in `image_format_properties`: {:?}", other);
                None
            }
        }
    }

    fn memory_properties(&self) -> adapter::MemoryProperties {
        let mem_properties = unsafe {
            self.instance
                .inner
                .get_physical_device_memory_properties(self.handle)
        };
        let memory_heaps = mem_properties.memory_heaps[..mem_properties.memory_heap_count as usize]
            .iter()
            .map(|mem| adapter::MemoryHeap {
                size: mem.size,
                flags: conv::map_vk_memory_heap_flags(mem.flags),
            })
            .collect();
        let memory_types = mem_properties.memory_types[..mem_properties.memory_type_count as usize]
            .iter()
            .filter_map(|mem| {
                if self.known_memory_flags.contains(mem.property_flags) {
                    Some(adapter::MemoryType {
                        properties: conv::map_vk_memory_properties(mem.property_flags),
                        heap_index: mem.heap_index as usize,
                    })
                } else {
                    warn!(
                        "Skipping memory type with unknown flags {:?}",
                        mem.property_flags
                    );
                    None
                }
            })
            .collect();

        adapter::MemoryProperties {
            memory_heaps,
            memory_types,
        }
    }

    fn features(&self) -> Features {
        // see https://github.com/gfx-rs/gfx/issues/1930
        let is_windows_intel_dual_src_bug = cfg!(windows)
            && self.properties.vendor_id == info::intel::VENDOR
            && (self.properties.device_id & info::intel::DEVICE_KABY_LAKE_MASK
                == info::intel::DEVICE_KABY_LAKE_MASK
                || self.properties.device_id & info::intel::DEVICE_SKY_LAKE_MASK
                    == info::intel::DEVICE_SKY_LAKE_MASK);

        let mut descriptor_indexing_features = None;
        let mut mesh_shader_features = None;
        let features = if let Some(ref get_device_properties) =
            self.instance.get_physical_device_properties
        {
            let features = vk::PhysicalDeviceFeatures::builder().build();
            let mut features2 = vk::PhysicalDeviceFeatures2KHR::builder()
                .features(features)
                .build();

            // Add extension infos to the p_next chain
            if self.supports_extension(vk::ExtDescriptorIndexingFn::name()) {
                descriptor_indexing_features =
                    Some(vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::builder().build());

                let mut_ref = descriptor_indexing_features.as_mut().unwrap();
                mut_ref.p_next = mem::replace(&mut features2.p_next, mut_ref as *mut _ as *mut _);
            }

            if self.supports_extension(MeshShader::name()) {
                mesh_shader_features =
                    Some(vk::PhysicalDeviceMeshShaderFeaturesNV::builder().build());

                let mut_ref = mesh_shader_features.as_mut().unwrap();
                mut_ref.p_next = mem::replace(&mut features2.p_next, mut_ref as *mut _ as *mut _);
            }

            unsafe {
                get_device_properties
                    .get_physical_device_features2_khr(self.handle, &mut features2 as *mut _);
            }
            features2.features
        } else {
            unsafe {
                self.instance
                    .inner
                    .get_physical_device_features(self.handle)
            }
        };

        let mut bits = Features::empty()
            | Features::TRIANGLE_FAN
            | Features::SEPARATE_STENCIL_REF_VALUES
            | Features::SAMPLER_MIP_LOD_BIAS
            | Features::SAMPLER_BORDER_COLOR
            | Features::MUTABLE_COMPARISON_SAMPLER
            | Features::MUTABLE_UNNORMALIZED_SAMPLER
            | Features::TEXTURE_DESCRIPTOR_ARRAY;

        if self.supports_extension(vk::AmdNegativeViewportHeightFn::name())
            || self.supports_extension(vk::KhrMaintenance1Fn::name())
        {
            bits |= Features::NDC_Y_UP;
        }
        if self.supports_extension(vk::KhrSamplerMirrorClampToEdgeFn::name()) {
            bits |= Features::SAMPLER_MIRROR_CLAMP_EDGE;
        }
        if self.supports_extension(DrawIndirectCount::name()) {
            bits |= Features::DRAW_INDIRECT_COUNT
        }
        // This will only be some if the extension exists
        if let Some(ref desc_indexing) = descriptor_indexing_features {
            if desc_indexing.shader_sampled_image_array_non_uniform_indexing != 0 {
                bits |= Features::SAMPLED_TEXTURE_DESCRIPTOR_INDEXING;
            }
            if desc_indexing.shader_storage_image_array_non_uniform_indexing != 0 {
                bits |= Features::STORAGE_TEXTURE_DESCRIPTOR_INDEXING;
            }
            if desc_indexing.runtime_descriptor_array != 0 {
                bits |= Features::UNSIZED_DESCRIPTOR_ARRAY;
            }
        }
        if let Some(ref mesh_shader) = mesh_shader_features {
            if mesh_shader.task_shader != 0 {
                bits |= Features::TASK_SHADER;
            }
            if mesh_shader.mesh_shader != 0 {
                bits |= Features::MESH_SHADER;
            }
        }

        if features.robust_buffer_access != 0 {
            bits |= Features::ROBUST_BUFFER_ACCESS;
        }
        if features.full_draw_index_uint32 != 0 {
            bits |= Features::FULL_DRAW_INDEX_U32;
        }
        if features.image_cube_array != 0 {
            bits |= Features::IMAGE_CUBE_ARRAY;
        }
        if features.independent_blend != 0 {
            bits |= Features::INDEPENDENT_BLENDING;
        }
        if features.geometry_shader != 0 {
            bits |= Features::GEOMETRY_SHADER;
        }
        if features.tessellation_shader != 0 {
            bits |= Features::TESSELLATION_SHADER;
        }
        if features.sample_rate_shading != 0 {
            bits |= Features::SAMPLE_RATE_SHADING;
        }
        if features.dual_src_blend != 0 && !is_windows_intel_dual_src_bug {
            bits |= Features::DUAL_SRC_BLENDING;
        }
        if features.logic_op != 0 {
            bits |= Features::LOGIC_OP;
        }
        if features.multi_draw_indirect != 0 {
            bits |= Features::MULTI_DRAW_INDIRECT;
        }
        if features.draw_indirect_first_instance != 0 {
            bits |= Features::DRAW_INDIRECT_FIRST_INSTANCE;
        }
        if features.depth_clamp != 0 {
            bits |= Features::DEPTH_CLAMP;
        }
        if features.depth_bias_clamp != 0 {
            bits |= Features::DEPTH_BIAS_CLAMP;
        }
        if features.fill_mode_non_solid != 0 {
            bits |= Features::NON_FILL_POLYGON_MODE;
        }
        if features.depth_bounds != 0 {
            bits |= Features::DEPTH_BOUNDS;
        }
        if features.wide_lines != 0 {
            bits |= Features::LINE_WIDTH;
        }
        if features.large_points != 0 {
            bits |= Features::POINT_SIZE;
        }
        if features.alpha_to_one != 0 {
            bits |= Features::ALPHA_TO_ONE;
        }
        if features.multi_viewport != 0 {
            bits |= Features::MULTI_VIEWPORTS;
        }
        if features.sampler_anisotropy != 0 {
            bits |= Features::SAMPLER_ANISOTROPY;
        }
        if features.texture_compression_etc2 != 0 {
            bits |= Features::FORMAT_ETC2;
        }
        if features.texture_compression_astc_ldr != 0 {
            bits |= Features::FORMAT_ASTC_LDR;
        }
        if features.texture_compression_bc != 0 {
            bits |= Features::FORMAT_BC;
        }
        if features.occlusion_query_precise != 0 {
            bits |= Features::PRECISE_OCCLUSION_QUERY;
        }
        if features.pipeline_statistics_query != 0 {
            bits |= Features::PIPELINE_STATISTICS_QUERY;
        }
        if features.vertex_pipeline_stores_and_atomics != 0 {
            bits |= Features::VERTEX_STORES_AND_ATOMICS;
        }
        if features.fragment_stores_and_atomics != 0 {
            bits |= Features::FRAGMENT_STORES_AND_ATOMICS;
        }
        if features.shader_tessellation_and_geometry_point_size != 0 {
            bits |= Features::SHADER_TESSELLATION_AND_GEOMETRY_POINT_SIZE;
        }
        if features.shader_image_gather_extended != 0 {
            bits |= Features::SHADER_IMAGE_GATHER_EXTENDED;
        }
        if features.shader_storage_image_extended_formats != 0 {
            bits |= Features::SHADER_STORAGE_IMAGE_EXTENDED_FORMATS;
        }
        if features.shader_storage_image_multisample != 0 {
            bits |= Features::SHADER_STORAGE_IMAGE_MULTISAMPLE;
        }
        if features.shader_storage_image_read_without_format != 0 {
            bits |= Features::SHADER_STORAGE_IMAGE_READ_WITHOUT_FORMAT;
        }
        if features.shader_storage_image_write_without_format != 0 {
            bits |= Features::SHADER_STORAGE_IMAGE_WRITE_WITHOUT_FORMAT;
        }
        if features.shader_uniform_buffer_array_dynamic_indexing != 0 {
            bits |= Features::SHADER_UNIFORM_BUFFER_ARRAY_DYNAMIC_INDEXING;
        }
        if features.shader_sampled_image_array_dynamic_indexing != 0 {
            bits |= Features::SHADER_SAMPLED_IMAGE_ARRAY_DYNAMIC_INDEXING;
        }
        if features.shader_storage_buffer_array_dynamic_indexing != 0 {
            bits |= Features::SHADER_STORAGE_BUFFER_ARRAY_DYNAMIC_INDEXING;
        }
        if features.shader_storage_image_array_dynamic_indexing != 0 {
            bits |= Features::SHADER_STORAGE_IMAGE_ARRAY_DYNAMIC_INDEXING;
        }
        if features.shader_clip_distance != 0 {
            bits |= Features::SHADER_CLIP_DISTANCE;
        }
        if features.shader_cull_distance != 0 {
            bits |= Features::SHADER_CULL_DISTANCE;
        }
        if features.shader_float64 != 0 {
            bits |= Features::SHADER_FLOAT64;
        }
        if features.shader_int64 != 0 {
            bits |= Features::SHADER_INT64;
        }
        if features.shader_int16 != 0 {
            bits |= Features::SHADER_INT16;
        }
        if features.shader_resource_residency != 0 {
            bits |= Features::SHADER_RESOURCE_RESIDENCY;
        }
        if features.shader_resource_min_lod != 0 {
            bits |= Features::SHADER_RESOURCE_MIN_LOD;
        }
        if features.sparse_binding != 0 {
            bits |= Features::SPARSE_BINDING;
        }
        if features.sparse_residency_buffer != 0 {
            bits |= Features::SPARSE_RESIDENCY_BUFFER;
        }
        if features.sparse_residency_image2_d != 0 {
            bits |= Features::SPARSE_RESIDENCY_IMAGE_2D;
        }
        if features.sparse_residency_image3_d != 0 {
            bits |= Features::SPARSE_RESIDENCY_IMAGE_3D;
        }
        if features.sparse_residency2_samples != 0 {
            bits |= Features::SPARSE_RESIDENCY_2_SAMPLES;
        }
        if features.sparse_residency4_samples != 0 {
            bits |= Features::SPARSE_RESIDENCY_4_SAMPLES;
        }
        if features.sparse_residency8_samples != 0 {
            bits |= Features::SPARSE_RESIDENCY_8_SAMPLES;
        }
        if features.sparse_residency16_samples != 0 {
            bits |= Features::SPARSE_RESIDENCY_16_SAMPLES;
        }
        if features.sparse_residency_aliased != 0 {
            bits |= Features::SPARSE_RESIDENCY_ALIASED;
        }
        if features.variable_multisample_rate != 0 {
            bits |= Features::VARIABLE_MULTISAMPLE_RATE;
        }
        if features.inherited_queries != 0 {
            bits |= Features::INHERITED_QUERIES;
        }

        bits
    }

    fn properties(&self) -> PhysicalDeviceProperties {
        let limits = {
            let limits = &self.properties.limits;

            let max_group_count = limits.max_compute_work_group_count;
            let max_group_size = limits.max_compute_work_group_size;

            Limits {
                max_image_1d_size: limits.max_image_dimension1_d,
                max_image_2d_size: limits.max_image_dimension2_d,
                max_image_3d_size: limits.max_image_dimension3_d,
                max_image_cube_size: limits.max_image_dimension_cube,
                max_image_array_layers: limits.max_image_array_layers as _,
                max_texel_elements: limits.max_texel_buffer_elements as _,
                max_patch_size: limits.max_tessellation_patch_size as PatchSize,
                max_viewports: limits.max_viewports as _,
                max_viewport_dimensions: limits.max_viewport_dimensions,
                max_framebuffer_extent: image::Extent {
                    width: limits.max_framebuffer_width,
                    height: limits.max_framebuffer_height,
                    depth: limits.max_framebuffer_layers,
                },
                max_compute_work_group_count: [
                    max_group_count[0] as _,
                    max_group_count[1] as _,
                    max_group_count[2] as _,
                ],
                max_compute_work_group_size: [
                    max_group_size[0] as _,
                    max_group_size[1] as _,
                    max_group_size[2] as _,
                ],
                max_vertex_input_attributes: limits.max_vertex_input_attributes as _,
                max_vertex_input_bindings: limits.max_vertex_input_bindings as _,
                max_vertex_input_attribute_offset: limits.max_vertex_input_attribute_offset as _,
                max_vertex_input_binding_stride: limits.max_vertex_input_binding_stride as _,
                max_vertex_output_components: limits.max_vertex_output_components as _,
                optimal_buffer_copy_offset_alignment: limits.optimal_buffer_copy_offset_alignment
                    as _,
                optimal_buffer_copy_pitch_alignment: limits.optimal_buffer_copy_row_pitch_alignment
                    as _,
                min_texel_buffer_offset_alignment: limits.min_texel_buffer_offset_alignment as _,
                min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment
                    as _,
                min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment
                    as _,
                framebuffer_color_sample_counts: limits.framebuffer_color_sample_counts.as_raw()
                    as _,
                framebuffer_depth_sample_counts: limits.framebuffer_depth_sample_counts.as_raw()
                    as _,
                framebuffer_stencil_sample_counts: limits.framebuffer_stencil_sample_counts.as_raw()
                    as _,
                timestamp_compute_and_graphics: limits.timestamp_compute_and_graphics != 0,
                max_color_attachments: limits.max_color_attachments as _,
                buffer_image_granularity: limits.buffer_image_granularity,
                non_coherent_atom_size: limits.non_coherent_atom_size as _,
                max_sampler_anisotropy: limits.max_sampler_anisotropy,
                min_vertex_input_binding_stride_alignment: 1,
                max_bound_descriptor_sets: limits.max_bound_descriptor_sets as _,
                max_compute_shared_memory_size: limits.max_compute_shared_memory_size as _,
                max_compute_work_group_invocations: limits.max_compute_work_group_invocations as _,
                descriptor_limits: DescriptorLimits {
                    max_per_stage_descriptor_samplers: limits.max_per_stage_descriptor_samplers,
                    max_per_stage_descriptor_storage_buffers: limits
                        .max_per_stage_descriptor_storage_buffers,
                    max_per_stage_descriptor_uniform_buffers: limits
                        .max_per_stage_descriptor_uniform_buffers,
                    max_per_stage_descriptor_sampled_images: limits
                        .max_per_stage_descriptor_sampled_images,
                    max_per_stage_descriptor_storage_images: limits
                        .max_per_stage_descriptor_storage_images,
                    max_per_stage_descriptor_input_attachments: limits
                        .max_per_stage_descriptor_input_attachments,
                    max_per_stage_resources: limits.max_per_stage_resources,
                    max_descriptor_set_samplers: limits.max_descriptor_set_samplers,
                    max_descriptor_set_uniform_buffers: limits.max_descriptor_set_uniform_buffers,
                    max_descriptor_set_uniform_buffers_dynamic: limits
                        .max_descriptor_set_uniform_buffers_dynamic,
                    max_descriptor_set_storage_buffers: limits.max_descriptor_set_storage_buffers,
                    max_descriptor_set_storage_buffers_dynamic: limits
                        .max_descriptor_set_storage_buffers_dynamic,
                    max_descriptor_set_sampled_images: limits.max_descriptor_set_sampled_images,
                    max_descriptor_set_storage_images: limits.max_descriptor_set_storage_images,
                    max_descriptor_set_input_attachments: limits
                        .max_descriptor_set_input_attachments,
                },
                max_draw_indexed_index_value: limits.max_draw_indexed_index_value,
                max_draw_indirect_count: limits.max_draw_indirect_count,
                max_fragment_combined_output_resources: limits
                    .max_fragment_combined_output_resources
                    as _,
                max_fragment_dual_source_attachments: limits.max_fragment_dual_src_attachments as _,
                max_fragment_input_components: limits.max_fragment_input_components as _,
                max_fragment_output_attachments: limits.max_fragment_output_attachments as _,
                max_framebuffer_layers: limits.max_framebuffer_layers as _,
                max_geometry_input_components: limits.max_geometry_input_components as _,
                max_geometry_output_components: limits.max_geometry_output_components as _,
                max_geometry_output_vertices: limits.max_geometry_output_vertices as _,
                max_geometry_shader_invocations: limits.max_geometry_shader_invocations as _,
                max_geometry_total_output_components: limits.max_geometry_total_output_components
                    as _,
                max_memory_allocation_count: limits.max_memory_allocation_count as _,
                max_push_constants_size: limits.max_push_constants_size as _,
                max_sampler_allocation_count: limits.max_sampler_allocation_count as _,
                max_sampler_lod_bias: limits.max_sampler_lod_bias as _,
                max_storage_buffer_range: limits.max_storage_buffer_range as _,
                max_uniform_buffer_range: limits.max_uniform_buffer_range as _,
                min_memory_map_alignment: limits.min_memory_map_alignment,
                standard_sample_locations: limits.standard_sample_locations == ash::vk::TRUE,
            }
        };

        let mut descriptor_indexing_capabilities = hal::DescriptorIndexingProperties::default();
        let mut mesh_shader_capabilities = hal::MeshShaderProperties::default();

        if let Some(get_physical_device_properties) =
            self.instance.get_physical_device_properties.as_ref()
        {
            let mut descriptor_indexing_properties =
                vk::PhysicalDeviceDescriptorIndexingPropertiesEXT::builder();
            let mut mesh_shader_properties = vk::PhysicalDeviceMeshShaderPropertiesNV::builder();

            unsafe {
                get_physical_device_properties.get_physical_device_properties2_khr(
                    self.handle,
                    &mut vk::PhysicalDeviceProperties2::builder()
                        .push_next(&mut mesh_shader_properties)
                        .push_next(&mut descriptor_indexing_properties)
                        .build() as *mut _,
                );
            }

            descriptor_indexing_capabilities = hal::DescriptorIndexingProperties {
                shader_uniform_buffer_array_non_uniform_indexing_native:
                    descriptor_indexing_properties
                        .shader_uniform_buffer_array_non_uniform_indexing_native
                        == vk::TRUE,
                shader_sampled_image_array_non_uniform_indexing_native:
                    descriptor_indexing_properties
                        .shader_sampled_image_array_non_uniform_indexing_native
                        == vk::TRUE,
                shader_storage_buffer_array_non_uniform_indexing_native:
                    descriptor_indexing_properties
                        .shader_storage_buffer_array_non_uniform_indexing_native
                        == vk::TRUE,
                shader_storage_image_array_non_uniform_indexing_native:
                    descriptor_indexing_properties
                        .shader_storage_image_array_non_uniform_indexing_native
                        == vk::TRUE,
                shader_input_attachment_array_non_uniform_indexing_native:
                    descriptor_indexing_properties
                        .shader_input_attachment_array_non_uniform_indexing_native
                        == vk::TRUE,
                quad_divergent_implicit_lod: descriptor_indexing_properties
                    .quad_divergent_implicit_lod
                    == vk::TRUE,
            };

            mesh_shader_capabilities = hal::MeshShaderProperties {
                max_draw_mesh_tasks_count: mesh_shader_properties.max_draw_mesh_tasks_count,
                max_task_work_group_invocations: mesh_shader_properties
                    .max_task_work_group_invocations,
                max_task_work_group_size: mesh_shader_properties.max_task_work_group_size,
                max_task_total_memory_size: mesh_shader_properties.max_task_total_memory_size,
                max_task_output_count: mesh_shader_properties.max_task_output_count,
                max_mesh_work_group_invocations: mesh_shader_properties
                    .max_mesh_work_group_invocations,
                max_mesh_work_group_size: mesh_shader_properties.max_mesh_work_group_size,
                max_mesh_total_memory_size: mesh_shader_properties.max_mesh_total_memory_size,
                max_mesh_output_vertices: mesh_shader_properties.max_mesh_output_vertices,
                max_mesh_output_primitives: mesh_shader_properties.max_mesh_output_primitives,
                max_mesh_multiview_view_count: mesh_shader_properties.max_mesh_multiview_view_count,
                mesh_output_per_vertex_granularity: mesh_shader_properties
                    .mesh_output_per_vertex_granularity,
                mesh_output_per_primitive_granularity: mesh_shader_properties
                    .mesh_output_per_primitive_granularity,
            };
        }

        PhysicalDeviceProperties {
            limits,
            descriptor_indexing: descriptor_indexing_capabilities,
            mesh_shader: mesh_shader_capabilities,
            performance_caveats: Default::default(),
            dynamic_pipeline_states: DynamicStates::all(),
        }
    }

    fn is_valid_cache(&self, cache: &[u8]) -> bool {
        const HEADER_SIZE: usize = 16 + vk::UUID_SIZE;

        if cache.len() < HEADER_SIZE {
            warn!("Bad cache data length {:?}", cache.len());
            return false;
        }

        let header_len = u32::from_le_bytes([cache[0], cache[1], cache[2], cache[3]]);
        let header_version = u32::from_le_bytes([cache[4], cache[5], cache[6], cache[7]]);
        let vendor_id = u32::from_le_bytes([cache[8], cache[9], cache[10], cache[11]]);
        let device_id = u32::from_le_bytes([cache[12], cache[13], cache[14], cache[15]]);

        // header length
        if (header_len as usize) < HEADER_SIZE {
            warn!("Bad header length {:?}", header_len);
            return false;
        }

        // cache header version
        if header_version != vk::PipelineCacheHeaderVersion::ONE.as_raw() as u32 {
            warn!("Unsupported cache header version: {:?}", header_version);
            return false;
        }

        // vendor id
        if vendor_id != self.properties.vendor_id {
            warn!(
                "Vendor ID mismatch. Device: {:?}, cache: {:?}.",
                self.properties.vendor_id, vendor_id,
            );
            return false;
        }

        // device id
        if device_id != self.properties.device_id {
            warn!(
                "Device ID mismatch. Device: {:?}, cache: {:?}.",
                self.properties.device_id, device_id,
            );
            return false;
        }

        if self.properties.pipeline_cache_uuid != cache[16..16 + vk::UUID_SIZE] {
            warn!(
                "Pipeline cache UUID mismatch. Device: {:?}, cache: {:?}.",
                self.properties.pipeline_cache_uuid,
                &cache[16..16 + vk::UUID_SIZE],
            );
            return false;
        }
        true
    }
}
