#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx12",
        feature = "dx11",
        feature = "metal",
        feature = "gl",
        feature = "gl-ci"
    )),
    allow(dead_code)
)]

extern crate gfx_warden as warden;
#[macro_use]
extern crate serde;

use hal::{adapter::PhysicalDevice as _, Instance as _};
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
}

type Suite = HashMap<String, HashMap<String, Test>>;

struct TestGroup {
    name: String,
    scene: warden::raw::Scene,
    tests: HashMap<String, Test>,
}

#[derive(Default)]
struct Disabilities {}

struct Harness {
    base_path: PathBuf,
    suite: Vec<TestGroup>,
}

impl Harness {
    fn new(suite_name: &str) -> Self {
        let base_path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../work"));
        println!("Parsing test suite '{}'...", suite_name);

        let suite_path = base_path
            .join("benches")
            .join(suite_name)
            .with_extension("ron");
        let suite = File::open(&suite_path)
            .map_err(de::Error::from)
            .and_then(de::from_reader::<_, Suite>)
            .expect(&format!("failed to open/parse the suite: {:?}", suite_path))
            .into_iter()
            .map(|(name, tests)| {
                let path = base_path.join("scenes").join(&name).with_extension("ron");
                let scene = File::open(path)
                    .map_err(de::Error::from)
                    .and_then(de::from_reader)
                    .expect("failed to open/parse the scene");
                TestGroup { name, scene, tests }
            })
            .collect();

        Harness { base_path, suite }
    }

    #[cfg_attr(any(feature = "gl", feature = "gl-ci"), allow(dead_code))]
    fn run<B: hal::Backend>(&self, name: &str, disabilities: Disabilities) {
        println!("Benching {}:", name);
        let instance = B::Instance::create("warden", 1).unwrap();
        self.run_instance(instance, disabilities)
    }

    fn run_instance<B: hal::Backend, I: hal::Instance<B>>(
        &self,
        instance: I,
        _disabilities: Disabilities,
    ) {
        for tg in &self.suite {
            let mut adapters = instance.enumerate_adapters();
            let adapter = adapters.remove(0);
            let features = adapter.physical_device.features();
            let limits = adapter.physical_device.limits();
            //println!("\t{:?}", adapter.info);
            println!("\tScene '{}':", tg.name);

            #[cfg(not(feature = "glsl-to-spirv"))]
            {
                let all_spirv = tg.scene.resources.values().all(|res| match *res {
                    warden::raw::Resource::Shader(ref name) => name.ends_with(".spirv"),
                    _ => true,
                });
                if !all_spirv {
                    println!("\t\tskipped {} tests (GLSL shaders)", tg.tests.len());
                    continue;
                }
            }

            let mut scene =
                warden::gpu::Scene::<B>::new(adapter, &tg.scene, self.base_path.join("data"))
                    .unwrap();

            for (test_name, test) in &tg.tests {
                print!("\t\tTest '{}' ...", test_name);
                if !features.contains(test.features) {
                    println!(
                        "\tskipped (features missing: {:?})",
                        test.features - features
                    );
                }
                let mut max_compute_work_groups = [0; 3];
                for job_name in &test.jobs {
                    if let warden::raw::Job::Compute { dispatch, .. } = tg.scene.jobs[job_name] {
                        for (max, count) in max_compute_work_groups.iter_mut().zip(dispatch.iter())
                        {
                            *max = (*max).max(*count);
                        }
                    }
                }
                if max_compute_work_groups[0] > limits.max_compute_work_group_size[0]
                    || max_compute_work_groups[1] > limits.max_compute_work_group_size[1]
                    || max_compute_work_groups[2] > limits.max_compute_work_group_size[2]
                {
                    println!("\tskipped (compute {:?})", max_compute_work_groups);
                    continue;
                }

                scene.run(test.jobs.iter());
                let time = scene.measure_time();
                println!(" {} mcs", time / 1000);
            }
        }
    }
}

fn main() {
    use std::env;

    #[cfg(feature = "env_logger")]
    env_logger::init();

    let suite_name = match env::args().nth(1) {
        Some(name) => name,
        None => {
            println!("Call with the argument of the bench suite name");
            return;
        }
    };

    let harness = Harness::new(&suite_name);
    #[cfg(feature = "vulkan")]
    {
        harness.run::<gfx_backend_vulkan::Backend>("Vulkan", Disabilities::default());
    }
    #[cfg(feature = "dx12")]
    {
        harness.run::<gfx_backend_dx12::Backend>("DX12", Disabilities::default());
    }
    #[cfg(feature = "dx11")]
    {
        harness.run::<gfx_backend_dx11::Backend>("DX11", Disabilities::default());
    }
    #[cfg(feature = "metal")]
    {
        harness.run::<gfx_backend_metal::Backend>("Metal", Disabilities::default());
    }
    #[cfg(feature = "gl")]
    {
        println!("Benching GL:");
        let instance = warden::init_gl_surface();
        harness.run_instance(instance, Disabilities::default());
    }
    #[cfg(feature = "gl-ci")]
    {
        println!("Benching GL on CI:");
        let instance = warden::init_gl_on_ci();
        harness.run_instance(instance, Disabilities::default());
    }
    #[cfg(not(any(
        feature = "vulkan",
        feature = "dx12",
        feature = "dx11",
        feature = "metal",
        feature = "gl",
        feature = "gl-ci"
    )))]
    {
        println!("No backend selected!");
        let _ = harness;
    }
}
