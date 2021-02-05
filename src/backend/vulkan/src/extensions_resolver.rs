use std::{
    collections::{HashMap, HashSet},
    ffi::CStr,
};

use ash::vk;

use crate::{PhysicalDevice, Version};

/// A declaration of an extension and its dependencies.
#[derive(Debug)]
pub(crate) struct Extension {
    name: &'static CStr,
    required_version: Version,
    dependencies: Vec<&'static CStr>,
    promoted_version: Option<Version>,
}

impl Extension {
    pub fn new(name: &'static CStr, required_version: Version) -> Self {
        Self {
            name,
            required_version,
            dependencies: Vec::new(),
            promoted_version: None,
        }
    }

    /// Add the (device extension) dependencies of this extension.
    pub fn with_dependencies(mut self, dependencies: Vec<&'static CStr>) -> Self {
        self.dependencies = dependencies;
        self
    }

    /// Add the api version in which this extension was promoted to core Vulkan.
    pub fn promoted_to(mut self, promoted_version: Version) -> Self {
        self.promoted_version = Some(promoted_version);
        self
    }

    /// Return `true` if this extension is compatible with `device_version` and can be requested when creating the device.
    pub fn is_compatible_with_version(&self, device_version: Version) -> bool {
        self.required_version.0
            <= vk::make_version(device_version.major(), device_version.minor(), 0)
    }

    /// Return `true` if this extension was promoted to core Vulkan in `device_version` and should not be explicitly requested when creating the device.
    pub fn is_promoted_by_version(&self, device_version: Version) -> bool {
        if let Some(promoted_version) = self.promoted_version {
            vk::make_version(device_version.major(), device_version.minor(), 0)
                >= promoted_version.0
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct CouldNotResolveExtensionsError;

/// Container to keep track of the extensions we use and their dependencies. Used for `Self::resolve_dependencies`.
#[derive(Debug)]
pub(crate) struct ExtensionsResolver {
    extensions: HashMap<&'static CStr, Extension>,
}

impl ExtensionsResolver {
    pub fn new<I>(registry: I) -> Self
    where
        I: IntoIterator<Item = Extension>,
    {
        Self {
            extensions: registry
                .into_iter()
                .map(|extension| (extension.name, extension))
                .collect(),
        }
    }

    /// Resolve `requested_extensions` into the full list of transitive dependencies and filter out any extensions that are not supported or are no longer explicitly needed.
    ///
    /// In general, `requested_extensions` should include the extension list needed to support the oldest version of Vulkan.
    ///
    /// If there is an extension that could not be resolved because:
    /// - The extension requires an instance version higher than the current instance version.
    /// - The extension is not supported by the device.
    ///
    /// ...this function will return `Err`.
    pub fn resolve_dependencies<I>(
        &self,
        physical_device: &PhysicalDevice,
        requested_extensions: I,
    ) -> Result<Vec<&'static CStr>, CouldNotResolveExtensionsError>
    where
        I: Iterator<Item = &'static CStr>,
    {
        let mut remaining = requested_extensions.collect::<Vec<_>>();
        let mut extensions = Vec::new();
        let mut marked = HashSet::new();

        let mut failed_to_resolve = false;

        while let Some(extension_name) = remaining.pop() {
            if let Some(extension) = self.extensions.get(extension_name) {
                if !extensions.contains(&extension.name) {
                    if !extension.is_compatible_with_version(physical_device.api_version) {
                        warn!(
                            "Extension {} (requires {:?}) is unsupported in version {:?}",
                            extension.name.to_string_lossy(),
                            extension.required_version,
                            physical_device.api_version
                        );
                        failed_to_resolve = true;
                        continue;
                    }

                    if extension.is_promoted_by_version(physical_device.api_version) {
                        // This extension was promoted to core, so we shouldn't request it.
                        debug!(
                            "Extension {} was promoted in {:?} and is no longer explicitly required.",
                            extension.name.to_string_lossy(),
                            extension.promoted_version,
                        );
                        continue;
                    }

                    // `VK_AMD_negative_viewport_height` is obsoleted by `VK_KHR_maintenance1`, so we should try to add that instead.
                    // Note this is the only extension we currently require that has this obsolescence deprecation state. If we gain more it may be worth refactoring `Extension` to support it.
                    if extension.name == vk::AmdNegativeViewportHeightFn::name()
                        && physical_device.supports_extension(vk::KhrMaintenance1Fn::name())
                    {
                        debug!(
                            "Extension {} was obsoleted by {}, which is supported by the device.",
                            extension.name.to_string_lossy(),
                            vk::KhrMaintenance1Fn::name().to_string_lossy()
                        );
                        remaining.push(vk::KhrMaintenance1Fn::name());
                        continue;
                    }

                    if physical_device.supports_extension(extension.name) {
                        extensions.push(extension.name);
                    } else {
                        warn!(
                            "Unsupported extension requested: {}",
                            extension.name.to_string_lossy()
                        );
                        failed_to_resolve = true;
                    }
                }

                // Ensure we don't walk the dependency graph more than once.
                if !marked.insert(extension.name) {
                    continue;
                }

                remaining.extend_from_slice(&extension.dependencies)
            } else {
                // If this trips, we have an unhandled extension that we need to add to track.
                unreachable!()
            }
        }

        if failed_to_resolve {
            Err(CouldNotResolveExtensionsError)
        } else {
            Ok(extensions)
        }
    }
}
