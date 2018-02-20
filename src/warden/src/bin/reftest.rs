#![cfg_attr(
    not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")),
    allow(dead_code)
)]

extern crate gfx_hal as hal;
extern crate gfx_warden as warden;
extern crate ron;
#[macro_use]
extern crate serde;

#[cfg(feature = "env_logger")]
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

use ron::de;


#[derive(Debug, Deserialize)]
enum Expectation {
    Buffer(String, Vec<u8>),
    ImageRow(String, usize, Vec<u8>),
}

#[derive(Debug, Deserialize)]
struct Test {
    features: hal::Features,
    jobs: Vec<String>,
    expect: Expectation,
}

type Suite = HashMap<String, HashMap<String, Test>>;

struct TestGroup {
    name: String,
    scene: warden::raw::Scene,
    tests: HashMap<String, Test>,
}

#[derive(Debug)]
struct TestResults {
    pass: usize,
    skip: usize,
    fail: usize,
}

#[derive(Default)]
struct Disabilities {
    no_command_buffer_reuse: bool,
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
        println!("Parsing test suite '{}'...", suite_name);

        let suite_path = base_path
            .join(suite_name)
            .with_extension("ron");
        let suite = File::open(suite_path)
            .map_err(de::Error::from)
            .and_then(de::from_reader::<_, Suite>)
            .expect("failed to open/parse the suite")
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

    fn run<I: hal::Instance>(
        &self,
        instance: I,
        disabilities: Disabilities,
    ) -> usize {
        use hal::{PhysicalDevice};

        let mut results = TestResults {
            pass: 0,
            skip: 0,
            fail: 0,
        };
        for tg in &self.suite {
            let mut adapters = instance.enumerate_adapters();
            let adapter = adapters.remove(0);
            let features = adapter.physical_device.features();
            let limits = adapter.physical_device.limits();
            //println!("\t{:?}", adapter.info);
            println!("\tScene '{}':", tg.name);

            #[cfg(not(feature = "glsl-to-spirv"))]
            {
                let all_spirv = tg.scene.resources
                    .values()
                    .all(|res| match *res {
                        warden::raw::Resource::Shader(ref name) => name.ends_with(".spirv"),
                        _ => true,
                    });
                if !all_spirv {
                    println!("\t\tskipped {} tests (GLSL shaders)", tg.tests.len());
                    results.skip += tg.tests.len();
                    continue
                }
            }

            let mut scene = warden::gpu::Scene::<I::Backend, _>::new(
                adapter,
                &tg.scene,
                self.base_path.join("data"),
            ).unwrap();

            for (test_name, test) in &tg.tests {
                print!("\t\tTest '{}' ...", test_name);
                if !features.contains(test.features) {
                    println!("\tskipped (features missing: {:?})", test.features - features);
                    results.skip += 1;
                }
                let mut max_compute_groups = [0; 3];
                for job_name in &test.jobs {
                    if let warden::raw::Job::Compute { dispatch, .. } = tg.scene.jobs[job_name] {
                         max_compute_groups[0] = max_compute_groups[0].max(dispatch.0);
                         max_compute_groups[1] = max_compute_groups[1].max(dispatch.1);
                         max_compute_groups[2] = max_compute_groups[2].max(dispatch.2);
                    }
                }
                if  max_compute_groups[0] > limits.max_compute_group_size[0] ||
                    max_compute_groups[1] > limits.max_compute_group_size[1] ||
                    max_compute_groups[2] > limits.max_compute_group_size[2]
                {
                    println!("\tskipped (compute {:?})", max_compute_groups);
                    results.skip += 1;
                    continue
                }

                scene.run(test.jobs.iter().map(|x| x.as_str()));

                print!("\tran: ");
                let (guard, row, data) = match test.expect {
                    Expectation::Buffer(ref buffer, ref data) =>
                        (scene.fetch_buffer(buffer), 0, data),
                    Expectation::ImageRow(ref image, row, ref data) =>
                        (scene.fetch_image(image), row, data),
                };

                if data.as_slice() == guard.row(row) {
                    println!("PASS");
                    results.pass += 1;
                } else {
                    println!("FAIL {:?}", guard.row(row));
                    results.fail += 1;
                }

                if disabilities.no_command_buffer_reuse {
                    println!("Command buffer re-use is not ready, exiting");
                    return results.fail;
                }
            }
        }

        println!("\t{:?}", results);
        results.fail
    }
}

fn main() {
    use std::{env, process};

    #[cfg(feature = "env_logger")]
    env_logger::init();
    let mut num_failures = 0;

    let suite_name = match env::args().nth(1) {
        Some(name) => name,
        None => {
            println!("Call with the argument of the reftest suite name");
            return
        }
    };

    let harness = Harness::new(&suite_name);
    #[cfg(feature = "vulkan")]
    {
        println!("Warding Vulkan:");
        let instance = gfx_backend_vulkan::Instance::create("warden", 1);
        num_failures += harness.run(instance, Disabilities::default());
    }
    #[cfg(feature = "dx12")]
    {
        println!("Warding DX12:");
        let instance = gfx_backend_dx12::Instance::create("warden", 1);
        num_failures += harness.run(instance, Disabilities::default());
    }
    #[cfg(feature = "metal")]
    {
        println!("Warding Metal:");
        let instance = gfx_backend_metal::Instance::create("warden", 1);
        num_failures += harness.run(instance, Disabilities {
            no_command_buffer_reuse: true,
            .. Disabilities::default()
        });
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
        num_failures += harness.run(instance, Disabilities::default());
    }
    #[cfg(feature = "gl-headless")]
    {
        println!("Warding GL headless:");
        let context = gfx_backend_gl::glutin::HeadlessRendererBuilder::new(1, 1)
            .build()
            .unwrap();
        let instance = gfx_backend_gl::Headless(context);
        num_failures += harness.run(instance, Disabilities::default());
    }
    let _ = harness;
    num_failures += 0; // mark as mutated
    process::exit(num_failures as _);
}
