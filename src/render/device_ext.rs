use std::ops;
use device;
use render::{batch, Renderer};
use render::shade::ShaderParam;
use device::shade::{Stage, CreateShaderError, ShaderModel};
use super::mesh::{Mesh, VertexFormat};

/// Program linking error
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ProgramError {
    /// Unable to compile the vertex shader
    Vertex(CreateShaderError),
    /// Unable to compile the fragment shader
    Fragment(CreateShaderError),
    /// Unable to link
    Link(()),
}

/// A type storing shader source for different graphics APIs and versions.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ShaderSource<'a> {
    pub glsl_120: Option<&'a [u8]>,
    pub glsl_130: Option<&'a [u8]>,
    pub glsl_140: Option<&'a [u8]>,
    pub glsl_150: Option<&'a [u8]>,
    pub glsl_430: Option<&'a [u8]>,
    // TODO: hlsl_sm_N...
    pub targets: &'a [&'a str],
}

impl<'a> ShaderSource<'a> {
    /// Create an empty shader source. Useful for specifying the remaining
    /// structure members upon construction.
    pub fn empty() -> ShaderSource<'a> {
        ShaderSource {
            glsl_120: None,
            glsl_130: None,
            glsl_140: None,
            glsl_150: None,
            glsl_430: None,
            targets: &[],
        }
    }

    /// Pick one of the stored versions that is the highest supported by the device.
    pub fn choose(&self, model: ShaderModel) -> Result<&'a [u8], ()> {
        // following https://www.opengl.org/wiki/Detecting_the_Shader_Model
        let version = model.to_number();
        Ok(match *self {
            ShaderSource { glsl_430: Some(s), .. } if version >= 50 => s,
            ShaderSource { glsl_150: Some(s), .. } if version >= 40 => s,
            ShaderSource { glsl_140: Some(s), .. } if version >= 40 => s,
            ShaderSource { glsl_130: Some(s), .. } if version >= 30 => s,
            ShaderSource { glsl_120: Some(s), .. } if version >= 20 => s,
            _ => return Err(()),
        })
    }
}


/// A convenient wrapper suitable for single-threaded operation.
pub struct Graphics<D: device::Device> {
    /// Graphics device.
    pub device: D,
    /// Renderer front-end.
    pub renderer: Renderer<D::Resources, D::CommandBuffer>,
    /// Hidden batch context.
    context: batch::Context<D::Resources>,
}

impl<D: device::Device> ops::Deref for Graphics<D> {
    type Target = batch::Context<D::Resources>;

    fn deref(&self) -> &batch::Context<D::Resources> {
        &self.context
    }
}

impl<D: device::Device> ops::DerefMut for Graphics<D> {
    fn deref_mut(&mut self) -> &mut batch::Context<D::Resources> {
        &mut self.context
    }
}


impl<D: device::Device> Graphics<D> {
    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ::ClearData, mask: ::Mask, frame: &::Frame<D::Resources>) {
        self.renderer.clear(data, mask, frame)
    }

    /// Draw a `RefBatch` batch.
    pub fn draw<'a, T: ShaderParam<Resources = D::Resources>>(&'a mut self,
                batch: &'a batch::RefBatch<T>, frame: &::Frame<D::Resources>)
                -> Result<(), ::DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&(batch, &self.context), frame)
    }

    /// Draw a `CoreBatch` batch.
    pub fn draw_core<'a, T: ShaderParam<Resources = D::Resources>>(&'a mut self,
                     core: &'a batch::CoreBatch<T>, slice: &'a ::Slice<D::Resources>,
                     params: &'a T, frame: &::Frame<D::Resources>)
                     -> Result<(), ::DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&self.context.bind(core, slice, params), frame)
    }

    /// Submit the internal command buffer and reset for the next frame.
    pub fn end_frame(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.renderer.reset();
    }
}


/// Backend extension trait for convenience methods
pub trait DeviceExt<R: device::Resources, C: device::draw::CommandBuffer<R>>:
    device::Factory<R> + device::Device<Resources = R, CommandBuffer = C>
{
    /// Create a new renderer
    fn create_renderer(&mut self) -> ::Renderer<R, C>;
    /// Convert to single-threaded wrapper
    fn into_graphics(mut self) -> Graphics<Self>;
}

impl<
    R: device::Resources,
    C: device::draw::CommandBuffer<R>,
    D: device::Factory<R> + device::Device<Resources = R, CommandBuffer = C>,
> DeviceExt<R, C> for D {
    fn create_renderer(&mut self) -> ::Renderer<R, C> {
        ::Renderer {
            command_buffer: device::draw::CommandBuffer::new(),
            data_buffer: device::draw::DataBuffer::new(),
            common_array_buffer: self.create_array_buffer(),
            draw_frame_buffer: self.create_frame_buffer(),
            read_frame_buffer: self.create_frame_buffer(),
            default_frame_buffer: self.get_main_frame_buffer(),
            render_state: super::RenderState::new(),
            parameters: super::ParamStorage::new(),
        }
    }

    fn into_graphics(mut self) -> Graphics<D> {
        let rend = self.create_renderer();
        Graphics {
            device: self,
            renderer: rend,
            context: batch::Context::new(),
        }
    }
}

/// Factory extension trait
pub trait FactoryExt<R: device::Resources> {
    /// Create a new mesh from the given vertex data.
    /// Convenience function around `create_buffer` and `Mesh::from_format`.
    fn create_mesh<T: VertexFormat + Copy>(&mut self, data: &[T]) -> Mesh<R>;
    /// Create a simple program given a vertex shader with a fragment one.
    fn link_program(&mut self, vs_code: &[u8], fs_code: &[u8])
                    -> Result<device::handle::Program<R>, ProgramError>;
    /// Create a simple program given `ShaderSource` versions of vertex and
    /// fragment shaders, chooss the matching versions for the device.
    fn link_program_source(&mut self, vs_src: ShaderSource, fs_src: ShaderSource,
                           caps: &device::Capabilities)
                           -> Result<device::handle::Program<R>, ProgramError>;
}

impl<R: device::Resources, F: device::Factory<R>> FactoryExt<R> for F {
    fn create_mesh<T: VertexFormat + Copy>(&mut self, data: &[T]) -> Mesh<R> {
        let nv = data.len();
        debug_assert!(nv < {
            use std::num::Int;
            let val: device::VertexCount = Int::max_value();
            val as usize
        });
        let buf = self.create_buffer_static(data);
        Mesh::from_format(buf, nv as device::VertexCount)
    }

    fn link_program(&mut self, vs_code: &[u8], fs_code: &[u8])
                    -> Result<device::handle::Program<R>, ProgramError> {
        let vs = match self.create_shader(Stage::Vertex, vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let fs = match self.create_shader(Stage::Fragment, fs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Fragment(e)),
        };

        self.create_program(&[vs, fs], None)
            .map_err(|e| ProgramError::Link(e))
    }

    fn link_program_source(&mut self, vs_src: ShaderSource, fs_src: ShaderSource,
                           caps: &device::Capabilities)
                           -> Result<device::handle::Program<R>, ProgramError> {
        let model = caps.shader_model;
        let err_model = CreateShaderError::ModelNotSupported;

        let vs = match vs_src.choose(model) {
            Ok(code) => match self.create_shader(Stage::Vertex, code) {
                Ok(s) => s,
                Err(e) => return Err(ProgramError::Vertex(e)),
            },
            Err(_) => return Err(ProgramError::Vertex(err_model))
        };

        let fs = match fs_src.choose(model) {
            Ok(code) => match self.create_shader(Stage::Fragment, code) {
                Ok(s) => s,
                Err(e) => return Err(ProgramError::Fragment(e)),
            },
            Err(_) => return Err(ProgramError::Fragment(err_model))
        };

        self.create_program(&[vs, fs], Some(fs_src.targets))
            .map_err(|e| ProgramError::Link(e))
    }
}
