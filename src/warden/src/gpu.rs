use std::collections::HashMap;

use hal::{self, Adapter};

use raw;


pub struct Resources<B: hal::Backend> {
    pub buffers: HashMap<String, B::Buffer>,
    pub images: HashMap<String, B::Image>,
}

pub struct Scene<B: hal::Backend> {
    pub resources: Resources<B>,
    pub jobs: HashMap<String, B::CommandBuffer>,
}

impl<B: hal::Backend> Scene<B> {
    pub fn new(adapter: &B::Adapter, raw: &raw::Scene) -> Self {
        // Build a new device and associated command queues
        let hal::Gpu { mut device, mut graphics_queues, memory_types, .. } = {
            let (ref family, queue_type) = adapter.get_queue_families()[0];
            assert!(queue_type.supports_graphics());
            adapter.open(&[(family, hal::QueueType::Graphics, 1)])
        };
        let mut queue = graphics_queues.remove(0);

        let mut resources = Resources {
            buffers: HashMap::new(),
            images: HashMap::new(),
        };
        let mut jobs = HashMap::new();
        Scene {
            resources,
            jobs,
        }
    }
}
