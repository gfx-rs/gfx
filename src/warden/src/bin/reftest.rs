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
#[cfg(any(feature = "gl", feature = "gl-headless"))]
extern crate gfx_backend_gl;

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::process;

use ron::de;


#[derive(Debug, Deserialize)]
enum Expectation {
    Buffer(String, Vec<u8>),
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
    base_path: PathBuf,
    suite: Vec<TestGroup>,
}

impl Harness {
    fn new(suite_name: &str) -> Self {
        let base_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reftests",
        ));
        println!("Parsing test suite...");

        let suite_path = base_path
            .join(suite_name)
            .with_extension("ron");
        let suite = File::open(suite_path)
            .map_err(de::Error::from)
            .and_then(de::from_reader::<_, Suite>)
            .expect("failed to parse the suite definition")
            .into_iter()
            .map(|(name, tests)| {
                let path = base_path
                    .join("scenes")
                    .join(&name)
                    .with_extension("ron");
                let scene = File::open(path)
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

    fn run<I: hal::Instance>(&self, instance: I) -> usize {
        let mut num_failures = 0;
        for tg in &self.suite {
            let mut adapters = instance.enumerate_adapters();
            let adapter = adapters.remove(0);
            //println!("\t{:?}", adapter.info);
            println!("\tScene '{}':", tg.name);

            let mut scene = warden::gpu::Scene::<I::Backend, _>::new(
                adapter,
                &tg.scene,
                &self.base_path.join("data"),
            ).unwrap();

            for (test_name, test) in &tg.tests {
                print!("\t\tTest '{}' ...", test_name);
                scene.run(test.jobs.iter().map(|x| x.as_str()));

                print!("\tran: ");
                let (guard, row, data) = match test.expect {
                    Expectation::Buffer(ref buffer, ref data) =>
                        (scene.fetch_buffer(buffer), 0, data),
                    Expectation::ImageRow(ref image, row, ref data) =>
                        (scene.fetch_image(image), row, data),
                };

                if data.as_slice() == guard.row(row) {
                    println!("PASS")
                } else {
                    println!("FAIL {:?}", guard.row(row));
                    num_failures += 1;
                }

                #[cfg(feature = "metal")]
                {
                    println!("Command buffer re-use is not ready on Metal, exiting");
                    return num_failures + 1;
                }
            }
        }
        num_failures
    }
}

fn main() {
    #[cfg(feature = "logger")]
    env_logger::init().unwrap();
    let mut num_failures = 0;

    let harness = Harness::new("suite");
    #[cfg(feature = "vulkan")]
    {
        println!("Warding Vulkan:");
        let instance = gfx_backend_vulkan::Instance::create("warden", 1);
        num_failures += harness.run(instance);
    }
    #[cfg(feature = "dx12")]
    {
        println!("Warding DX12:");
        let instance = gfx_backend_dx12::Instance::create("warden", 1);
        num_failures += harness.run(instance);
    }
    #[cfg(feature = "metal")]
    {
        println!("Warding Metal:");
        let instance = gfx_backend_metal::Instance::create("warden", 1);
        num_failures += harness.run(instance);
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
        num_failures += harness.run(instance);
    }
    #[cfg(feature = "gl-headless")]
    {
        println!("Warding GL headless:");
        let context = gfx_backend_gl::glutin::HeadlessRendererBuilder::new(1, 1)
            .build()
            .unwrap();
        let instance = gfx_backend_gl::Headless(context);
        num_failures += harness.run(instance);
    }
    let _ = harness;
    process::exit(num_failures as _);
}
