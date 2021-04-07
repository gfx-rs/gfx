use ash::vk;
use hal::{
    adapter,
    display,
};

use crate::{Instance,Backend,native,window};

impl Instance
{

}


pub fn vk_transformations_to_hal(vk_transformations: vk::SurfaceTransformFlagsKHR)->Vec<display::SurfaceTransformation>
{
    let mut transformations = Vec::new();
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::IDENTITY) {transformations.push(display::SurfaceTransformation::Identity);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::ROTATE_90) {transformations.push(display::SurfaceTransformation::Rotate90);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::ROTATE_180) {transformations.push(display::SurfaceTransformation::Rotate180);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::ROTATE_270) {transformations.push(display::SurfaceTransformation::Rotate270);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR) {transformations.push(display::SurfaceTransformation::HorizontalMirror);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_90) {transformations.push(display::SurfaceTransformation::HorizontalMirrorRotate90);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_180) {transformations.push(display::SurfaceTransformation::HorizontalMirrorRotate180);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_270) {transformations.push(display::SurfaceTransformation::HorizontalMirrorRotate270);}
    if vk_transformations.contains(vk::SurfaceTransformFlagsKHR::INHERIT) {transformations.push(display::SurfaceTransformation::Inherit);}
    return transformations;
}
/*
fn vk_transformations_to_hal(vk_transformations: vk::SurfaceTransformFlagsKHR)->display::SurfaceTransformations
{
    display::SurfaceTransformations {
        identity: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::IDENTITY),
        rotate_90: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::ROTATE_90),
        rotate_180: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::ROTATE_180),
        rotate_270: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::ROTATE_270),
        horizontal_mirror: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR),
        horizontal_mirror_rotate_90: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_90),
        horizontal_mirror_rotate_180: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_180),
        horizontal_mirror_rotate_270: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_270),
        inherit: vk_transformations.contains(vk::SurfaceTransformFlagsKHR::INHERIT)
    }
}

fn hal_transformations_to_vk(hal_transformations: display::SurfaceTransformations)->vk::SurfaceTransformFlagsKHR
{
    let mut vk_transformations = vk::SurfaceTransformFlagsKHR::default();
    if hal_transformations.identity {vk_transformations |= vk::SurfaceTransformFlagsKHR::IDENTITY;}
    if hal_transformations.rotate_90 {vk_transformations |= vk::SurfaceTransformFlagsKHR::ROTATE_90;}
    if hal_transformations.rotate_180 {vk_transformations |= vk::SurfaceTransformFlagsKHR::ROTATE_180;}
    if hal_transformations.rotate_270 {vk_transformations |= vk::SurfaceTransformFlagsKHR::ROTATE_270;}
    if hal_transformations.horizontal_mirror {vk_transformations |= vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR;}
    if hal_transformations.horizontal_mirror_rotate_90 {vk_transformations |= vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_90;}
    if hal_transformations.horizontal_mirror_rotate_180 {vk_transformations |= vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_180;}
    if hal_transformations.horizontal_mirror_rotate_270 {vk_transformations |= vk::SurfaceTransformFlagsKHR::HORIZONTAL_MIRROR_ROTATE_270;}
    if hal_transformations.inherit {vk_transformations |= vk::SurfaceTransformFlagsKHR::INHERIT;}
    return vk_transformations;
}
*/
/*
impl std::convert::From<vk::SurfaceTransformFlagsKHR> for display::SupportedTransforms
{
    fn from(supported_transform: vk::SurfaceTransformFlagsKHR)->Self
    {

    }
}
*/
#[test]
fn test_vulkan_display()
{
    use hal::Instance as HalInstance;
    let instance = Instance::create("", 2).unwrap();
    let adapters = instance.enumerate_adapters();
    println!("Adapters found: {:#?}",&adapters);
    unsafe{instance.enumerate_displays(&adapters[0]);}
}
