#[path = "build/match_algo.rs"]
mod match_algo;
#[path = "build/gen_parsers.rs"]
mod gen_parsers;

use std::path::Path;

fn main() {
    let schema_dir = Path::new("../kozan-style/schema");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out = Path::new(&out_dir);

    let types = kozan_build_utils::load_types(schema_dir);
    let groups = kozan_build_utils::load_property_groups(schema_dir);

    let parsers_code = gen_parsers::generate(&types, &groups);
    std::fs::write(out.join("generated_parsers.rs"), parsers_code)
        .expect("failed to write generated_parsers.rs");

    println!("cargo::rerun-if-changed=../kozan-style/schema/");
}
