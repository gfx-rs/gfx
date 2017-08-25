use winapi::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Buffer(pub *mut ID3D11Buffer);
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Texture {
    D1(*mut ID3D11Texture1D),
    D2(*mut ID3D11Texture2D),
    D3(*mut ID3D11Texture3D),
}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Rtv(pub *mut ID3D11RenderTargetView);
unsafe impl Send for Rtv {}
unsafe impl Sync for Rtv {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Dsv(pub *mut ID3D11DepthStencilView);
unsafe impl Send for Dsv {}
unsafe impl Sync for Dsv {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Srv(pub *mut ID3D11ShaderResourceView);
unsafe impl Send for Srv {}
unsafe impl Sync for Srv {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Sampler(pub *mut ID3D11SamplerState);
unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}
