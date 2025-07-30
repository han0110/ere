use build_utils::{detect_ere_version, detect_sdk_versions};
use std::{env, fs, path::Path};

fn main() {
    generate_ere_version();
    generate_zkvm_sdk_version_impl();
    println!("cargo:rerun-if-changed=Cargo.lock");
}

fn generate_ere_version() {
    let ere_version = format!("const ERE_VERSION: &str = \"{}\";", detect_ere_version());

    let out_dir = env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("ere_version.rs");
    fs::write(dst, ere_version).unwrap();
}

fn generate_zkvm_sdk_version_impl() {
    let [
        jolt_version,
        nexus_version,
        openvm_version,
        pico_version,
        risc0_version,
        sp1_version,
    ] = detect_sdk_versions([
        "jolt-sdk",
        "nexus-sdk",
        "openvm-sdk",
        "pico-sdk",
        "risc0-zkvm",
        "sp1-sdk",
    ])
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();
    let zisk_version = "0.9.0";

    let zkvm_sdk_version_impl = format!(
        r#"impl crate::ErezkVM {{
    pub fn sdk_version(&self) -> &'static str {{
        match self {{
            Self::Jolt => "{jolt_version}",
            Self::Nexus => "{nexus_version}",
            Self::OpenVM => "{openvm_version}",
            Self::Pico => "{pico_version}",
            Self::Risc0 => "{risc0_version}",
            Self::SP1 => "{sp1_version}",
            Self::Zisk => "{zisk_version}",
        }}
    }}
}}"#,
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("zkvm_sdk_version_impl.rs");
    fs::write(dst, zkvm_sdk_version_impl).unwrap();
}
