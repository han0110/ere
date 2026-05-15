use std::{env, path::PathBuf};

const DIR_FOR_HEADER: &str = "build";

fn main() {
    println!("cargo:rerun-if-changed=src/");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR env not set");
    let path_to_crate_dir = PathBuf::from(&crate_dir);

    let output_file = path_to_crate_dir
        .join(DIR_FOR_HEADER)
        .join("ere_verifier.h")
        .display()
        .to_string();

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("ERE_VERIFIER_H")
        .with_documentation(true)
        .with_pragma_once(true)
        .generate()
        .expect("cbindgen failed to generate ere_verifier.h")
        .write_to_file(output_file);
}
