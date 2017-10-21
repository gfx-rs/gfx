#![cfg_attr(
    not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")),
    allow(dead_code)
)]

extern crate gfx_hal as hal;
extern crate gfx_warden as warden;
extern crate ron;
#[macro_use]
extern crate serde;

#[cfg(feature = "logger")]
extern crate env_logger;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl;

use std::collections::HashMap;
use std::fs::File;

use ron::de;


#[derive(Debug, Deserialize)]
enum Expectation {
    ImageRow(String, usize, Vec<u8>),
}

#[derive(Debug, Deserialize)]
struct Test {
    jobs: Vec<String>,
    expect: Expectation,
}

type Suite = HashMap<String, HashMap<String, Test>>;


struct Harness {
    base_path: &'static str,
    suite: Suite,
}

impl Harness {
    fn new(suite_name: &str) -> Self {
        let base_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reftests",
        );
        let suite = File::open(format!("{}/{}.ron", base_path, suite_name))
            .map_err(de::Error::from)
            .and_then(de::from_reader)
            .expect("failed to parse the suite definition");
        Harness {
            base_path,
            suite,
        }
    }

    fn run<I: hal::Instance>(&self, instance: I) {
        use hal::Adapter;

        let adapters = instance.enumerate_adapters();
        let adapter = &adapters[0];
        println!("\t{:?}", adapter.info());

        for (scene_name, tests) in &self.suite {
            println!("\tLoading scene '{}':", scene_name);
            let raw_scene = File::open(format!("{}/scenes/{}.ron", self.base_path, scene_name))
                .map_err(de::Error::from)
                .and_then(de::from_reader)
                .expect("failed to open/parse the scene");

            let data_path = format!("{}/data", self.base_path);
            let mut scene = warden::gpu::Scene::<I::Backend>::new(adapter, &raw_scene, &data_path);

            for (test_name, test) in tests {
                print!("\t\tTest '{}' ...", test_name);
                scene.run(test.jobs.iter().map(|x| x.as_str()));

                print!("\tran: ");
                match test.expect {
                    Expectation::ImageRow(ref image, row, ref data) => {
                        let guard = scene.fetch_image(image);
                        if data.as_slice() == guard.row(row) {
                            println!("PASS");
                        } else {
                            println!("FAIL {:?}", guard.row(row));
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    #[cfg(feature = "logger")]
    env_logger::init().unwrap();

    let harness = Harness::new("suite");
    #[cfg(feature = "vulkan")]
    {
        println!("Warding Vulkan:");
        let instance = gfx_backend_vulkan::Instance::create("warden", 1);
        harness.run(instance);
    }
    #[cfg(feature = "dx12")]
    {
        println!("Warding DX12:");
        let instance = gfx_backend_dx12::Instance::create("warden", 1);
        harness.run(instance);
    }
    #[cfg(feature = "metal")]
    {
        println!("Warding Metal:");
        let instance = gfx_backend_metal::Instance::create("warden", 1);
        harness.run(instance);
    }
    #[cfg(feature = "gl")]
    {
        println!("Warding GL:");
        let context = gfx_backend_gl::glutin::HeadlessRendererBuilder::new(1,1)
            .build()
            .unwrap();
        let instance = gfx_backend_gl::Headless(context);
        harness.run(instance);
    }
    let _ = harness;
}
