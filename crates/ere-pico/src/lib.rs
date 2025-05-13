use std::process::Command;
use zkvm_interface::Compiler;

mod error;
use error::PicoError;

#[allow(non_camel_case_types)]
pub struct PICO_TARGET;

impl Compiler for PICO_TARGET {
    type Error = PicoError;

    type Program = Vec<u8>;

    fn compile(path: &std::path::Path) -> Result<Self::Program, Self::Error> {
        // 1. Check guest path
        if !path.exists() {
            return Err(PicoError::PathNotFound(path.to_path_buf()));
        }

        // 2. Run `cargo pico build`
        let status = Command::new("cargo")
            .current_dir(path)
            .env("RUST_LOG", "info")
            .args(["pico", "build"])
            .status()?; // From<io::Error> → Spawn

        if !status.success() {
            return Err(PicoError::CargoFailed { status });
        }

        // 3. Locate the ELF file
        let elf_path = path
            .parent()
            .expect("guest dir always has a parent")
            .join("elf/riscv32im-pico-zkvm-elf");

        if !elf_path.exists() {
            return Err(PicoError::ElfNotFound(elf_path));
        }

        // 4. Read the ELF file
        let elf_bytes = std::fs::read(&elf_path).map_err(|e| PicoError::ReadElf {
            path: elf_path,
            source: e,
        })?;

        Ok(elf_bytes)
    }
}

#[cfg(test)]
mod tests {
    use zkvm_interface::Compiler;

    use crate::PICO_TARGET;

    use super::*;
    use std::path::PathBuf;

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("pico")
            .join("compile")
            .join("basic")
            // TODO: Refactor the basic test to not have a lib and app dir
            .join("app")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/pico")
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        match PICO_TARGET::compile(&test_guest_path) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(e) => {
                panic!(
                    "compile_sp1_program direct call failed for dedicated guest: {:?}",
                    e
                );
            }
        }
    }
}
