use ash::vk;

#[derive(Debug, Hash)]
pub struct Image(pub vk::Image);

#[derive(Debug, Hash)]
pub struct Semaphore(pub vk::Semaphore);

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Fence(pub vk::Fence);
