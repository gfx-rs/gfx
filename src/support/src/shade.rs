
use std::error::Error;
use std::fmt;

#[cfg(feature = "gl")]
pub use gfx_device_gl::Version as GlslVersion;
#[cfg(feature = "metal")]
pub use gfx_device_metal::ShaderModel as MetalShaderModel;

pub type DxShaderModel = u16;

/// Shader backend with version numbers.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Backend {
    #[cfg(feature = "gl")]
    Glsl(GlslVersion),
    #[cfg(feature = "gl")]
    GlslEs(GlslVersion),
    #[cfg(any(feature = "dx11", feature = "dx12"))]
    Hlsl(DxShaderModel),
    #[cfg(feature = "metal")]
    Msl(MetalShaderModel),
    #[cfg(feature = "vulkan")]
    Vulkan,
}

pub trait ShadeExt {
    fn shader_backend(&self) -> Backend;
}

#[cfg(feature = "gl")]
impl ShadeExt for ::gfx_device_gl::Device {
    fn shader_backend(&self) -> Backend {
        let shade_lang = self.get_info().shading_language;
        if shade_lang.is_embedded {
            Backend::GlslEs(shade_lang)
        } else {
            Backend::Glsl(shade_lang)
        }
    }
}

#[cfg(feature = "dx11")]
impl ShadeExt for ::gfx_device_dx11::Device {
    fn shader_backend(&self) -> Backend {
        Backend::Hlsl(self.shader_model())
    }
}

#[cfg(feature = "dx12")]
impl ShadeExt for ::gfx_device_dx12::Device {
    fn shader_backend(&self) -> Backend {
        Backend::Hlsl(self.shader_model())
    }
}

#[cfg(feature = "metal")]
impl ShadeExt for ::gfx_device_metal::Device {
    fn shader_backend(&self) -> Backend {
        Backend::Msl(self.shader_model())
    }
}

#[cfg(feature = "vulkan")]
impl ShadeExt for ::gfx_device_vulkan::Device {
    fn shader_backend(&self) -> Backend {
        Backend::Vulkan
    }
}

pub const EMPTY: &'static [u8] = &[];

/// A type storing shader source for different graphics APIs and versions.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Source<'a> {
    pub glsl_120: &'a [u8],
    pub glsl_130: &'a [u8],
    pub glsl_140: &'a [u8],
    pub glsl_150: &'a [u8],
    pub glsl_400: &'a [u8],
    pub glsl_430: &'a [u8],
    pub glsl_es_100: &'a [u8],
    pub glsl_es_200: &'a [u8],
    pub glsl_es_300: &'a [u8],
    pub hlsl_30: &'a [u8],
    pub hlsl_40: &'a [u8],
    pub hlsl_41: &'a [u8],
    pub hlsl_50: &'a [u8],
    pub msl_10: &'a [u8],
    pub msl_11: &'a [u8],
    pub vulkan: &'a [u8],
}

/// Error selecting a backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectError(Backend);

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "An error occurred when selecting the {:?} backend", self.0)
    }
}

impl Error for SelectError {
    fn description(&self) -> &str {
        "An error occurred when selecting a backend"
    }
}

impl<'a> Source<'a> {
    /// Create an empty shader source. Useful for specifying the remaining
    /// structure members upon construction.
    pub fn empty() -> Source<'a> {
        Source {
            glsl_120: EMPTY,
            glsl_130: EMPTY,
            glsl_140: EMPTY,
            glsl_150: EMPTY,
            glsl_400: EMPTY,
            glsl_430: EMPTY,
            glsl_es_100: EMPTY,
            glsl_es_200: EMPTY,
            glsl_es_300: EMPTY,
            hlsl_30: EMPTY,
            hlsl_40: EMPTY,
            hlsl_41: EMPTY,
            hlsl_50: EMPTY,
            msl_10: EMPTY,
            msl_11: EMPTY,
            vulkan: EMPTY,
        }
    }

    /// Pick one of the stored versions that is the highest supported by the backend.
    pub fn select(&self, backend: Backend) -> Result<&'a [u8], SelectError> {
        Ok(match backend {
            #[cfg(feature = "gl")]
            Backend::Glsl(version) => {
                let v = version.major * 100 + version.minor;
                match *self {
                    Source { glsl_430: s, .. } if s != EMPTY && v >= 430 => s,
                    Source { glsl_400: s, .. } if s != EMPTY && v >= 400 => s,
                    Source { glsl_150: s, .. } if s != EMPTY && v >= 150 => s,
                    Source { glsl_140: s, .. } if s != EMPTY && v >= 140 => s,
                    Source { glsl_130: s, .. } if s != EMPTY && v >= 130 => s,
                    Source { glsl_120: s, .. } if s != EMPTY && v >= 120 => s,
                    _ => return Err(SelectError(backend)),
                }
            }
            #[cfg(feature = "gl")]
            Backend::GlslEs(version) => {
                let v = version.major * 100 + version.minor;
                match *self {
                    Source { glsl_es_100: s, .. } if s != EMPTY && v >= 100 => s,
                    Source { glsl_es_200: s, .. } if s != EMPTY && v >= 200 => s,
                    Source { glsl_es_300: s, .. } if s != EMPTY && v >= 300 => s,
                    _ => return Err(SelectError(backend)),
                }
            }
            #[cfg(feature = "dx11")]
            Backend::Hlsl(model) => {
                match *self {
                    Source { hlsl_50: s, .. } if s != EMPTY && model >= 50 => s,
                    Source { hlsl_41: s, .. } if s != EMPTY && model >= 41 => s,
                    Source { hlsl_40: s, .. } if s != EMPTY && model >= 40 => s,
                    Source { hlsl_30: s, .. } if s != EMPTY && model >= 30 => s,
                    _ => return Err(SelectError(backend)),
                }
            }
            #[cfg(feature = "dx12")]
            Backend::Hlsl(model) => {
                return Err(SelectError(backend)) // TODO
            }
            #[cfg(feature = "metal")]
            Backend::Msl(revision) => {
                match *self {
                    Source { msl_11: s, .. } if s != EMPTY && revision >= 11 => s,
                    Source { msl_10: s, .. } if s != EMPTY && revision >= 10 => s,
                    _ => return Err(SelectError(backend)),
                }
            }
            #[cfg(feature = "vulkan")]
            Backend::Vulkan => {
                match *self {
                    Source { vulkan: s, .. } if s != EMPTY => s,
                    _ => return Err(SelectError(backend)),
                }
            }
        })
    }
}
