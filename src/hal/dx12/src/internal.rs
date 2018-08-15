use hal::backend::FastHashMap;
use std::sync::Mutex;

use bal_dx12::blit::{BlitKey, BlitPipe};
use bal_dx12::native;

type BlitMap = FastHashMap<BlitKey, BlitPipe>;

pub(crate) struct ServicePipes {
    pub(crate) device: native::Device,
    blits_2d_color: Mutex<BlitMap>,
}

impl ServicePipes {
    pub fn new(device: native::Device) -> Self {
        ServicePipes {
            device,
            blits_2d_color: Mutex::new(BlitMap::default()),
        }
    }

    pub unsafe fn destroy(&self) {
        let blits = self.blits_2d_color.lock().unwrap();
        for (_, pipe) in &*blits {
            pipe.destroy();
        }
    }

    pub fn get_blit_2d_color(&self, key: BlitKey) -> BlitPipe {
        let mut blits = self.blits_2d_color.lock().unwrap();
        blits
            .entry(key)
            .or_insert_with(|| BlitPipe::new(self.device, key))
            .clone()
    }
}
