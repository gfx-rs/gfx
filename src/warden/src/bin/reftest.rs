extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;
extern crate gfx_warden as warden;
extern crate ron;
#[macro_use]
extern crate serde;

use std::collections::HashMap;
use std::fs::File;

use hal::{Adapter, Instance};
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


fn main() {
    let base_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reftests",
    );
    let data_path = format!("{}/data", base_path);

    let instance = back::Instance::create("warden", 1);
    let adapters = instance.enumerate_adapters();
    println!("Initialized graphics with {:#?}", adapters[0].get_info());

    let suite: Suite = File::open(format!("{}/suite.ron", base_path))
        .map_err(de::Error::from)
        .and_then(de::from_reader)
        .expect("failed to parse the suite definition");

    for (scene_name, tests) in suite {
        println!("Loading scene '{}'", scene_name);
        let raw_scene = File::open(format!("{}/scenes/{}.ron", base_path, scene_name))
            .map_err(de::Error::from)
            .and_then(de::from_reader)
            .expect("failed to open/parse the scene");

        let mut scene = warden::gpu::Scene::<back::Backend>::new(&adapters[0], &raw_scene, &data_path);

        for (test_name, test) in tests {
            print!("\tTest '{}' ... ", test_name);
            scene.run(test.jobs.iter().map(|x| x.as_str()));

            print!("ran; expectation: ");
            match test.expect {
                Expectation::ImageRow(image, row, data) => {
                    let guard = scene.fetch_image(&image);
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
