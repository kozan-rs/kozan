#[path = "build/gen_types.rs"]
mod gen_types;
#[path = "build/gen_properties.rs"]
mod gen_properties;
#[path = "build/gen_builder.rs"]
mod gen_builder;

use std::path::Path;

fn main() {
    let schema_dir = Path::new("schema");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out = Path::new(&out_dir);

    let types = kozan_build_utils::load_types(schema_dir);
    let groups = kozan_build_utils::load_property_groups(schema_dir);

    let types_code = gen_types::generate(&types);
    std::fs::write(out.join("generated_types.rs"), types_code)
        .expect("failed to write generated_types.rs");

    let props_code = gen_properties::generate(&groups);
    std::fs::write(out.join("generated_properties.rs"), props_code)
        .expect("failed to write generated_properties.rs");

    let builder_code = gen_builder::generate(&groups);
    std::fs::write(out.join("generated_builder.rs"), builder_code)
        .expect("failed to write generated_builder.rs");

    println!("cargo::rerun-if-changed=schema/");
}
