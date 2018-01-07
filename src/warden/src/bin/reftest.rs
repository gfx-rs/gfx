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

struct TestGroup {
    name: String,
    scene: warden::raw::Scene,
    tests: HashMap<String, Test>,
}


struct Harness {
    base_path: &'static str,
    suite: Vec<TestGroup>,
}

impl Harness {
    fn new(suite_name: &str) -> Self {
        let base_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reftests",
        );
        println!("Parsing test suite...");

        let suite = File::open(format!("{}/{}.ron", base_path, suite_name))
            .map_err(de::Error::from)
            .and_then(de::from_reader::<_, Suite>)
            .expect("failed to parse the suite definition")
            .into_iter()
            .map(|(name, tests)| {
                let scene = File::open(format!("{}/scenes/{}.ron", base_path, name))
                    .map_err(de::Error::from)
                    .and_then(de::from_reader)
                    .expect("failed to open/parse the scene");
                TestGroup {
                    name,
                    scene,
                    tests,
                }
            })
            .collect();

        Harness {
            base_path,
            suite,
        }
    }

    fn run<I: hal::Instance>(&self, instance: I) {
        for tg in &self.suite {
            let mut adapters = instance.enumerate_adapters();
            let adapter = adapters.remove(0);
            //println!("\t{:?}", adapter.info);
            println!("\tScene '{}':", tg.name);

            let data_path = format!("{}/data", self.base_path);
            let mut scene = warden::gpu::Scene::<I::Backend>::new(
                adapter,
                &tg.scene,
                &data_path,
            ).unwrap();

            for (test_name, test) in &tg.tests {
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
        use gfx_backend_gl::glutin;
        println!("Warding GL:");
        let events_loop = glutin::EventsLoop::new();
        let window = glutin::GlWindow::new(
            glutin::WindowBuilder::new(),
            glutin::ContextBuilder::new()
                .with_gl_profile(glutin::GlProfile::Core),
            &events_loop,
            ).unwrap();
        let instance = gfx_backend_gl::Surface::from_window(window);
        harness.run(instance);
    }
    #[cfg(feature = "gl-soft")]
    {
        println!("Warding GL software:");
        let context = gfx_backend_gl::glutin::HeadlessRendererBuilder::new(1, 1)
            .build()
            .unwrap();
        let instance = gfx_backend_gl::Headless(context);
        harness.run(instance);
    }
    let _ = harness;
}
