use hal::backend::FastHashMap;
use hal::pso;
use spirv_cross::hlsl;
use std::sync::Mutex;
use std::{mem, ptr};

use d3d12;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::{dxgiformat, dxgitype, winerror};
use winapi::um::d3d12::*;
use winapi::Interface;

use device;

use bal_dx12::blit::{BlitKey, BlitPipe};
use bal_dx12::native;
use bal_dx12::native::descriptor;

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
            .or_insert_with(|| self.create_blit_2d_color(key))
            .clone()
    }
}
