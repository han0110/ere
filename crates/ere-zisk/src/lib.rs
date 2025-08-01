use crate::{
    compile::compile_zisk_program,
    error::{ExecuteError, ProveError, VerifyError, ZiskError},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time,
};
use tempfile::{TempDir, tempdir};
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, ProverResourceType, zkVM,
    zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod compile;
mod error;

#[allow(non_camel_case_types)]
pub struct RV64_IMA_ZISK_ZKVM_ELF;

impl Compiler for RV64_IMA_ZISK_ZKVM_ELF {
    type Error = ZiskError;

    type Program = Vec<u8>;

    fn compile(
        workspace_directory: &Path,
        guest_relative: &Path,
    ) -> Result<Self::Program, Self::Error> {
        compile_zisk_program(&workspace_directory.join(guest_relative)).map_err(ZiskError::Compile)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ZiskProofWithPublicValues {
    /// The raw aggregated proof generated by the ZisK zkVM.
    pub proof: Vec<u8>,
    /// The public values generated by the ZisK zkVM.
    pub public_values: Vec<u8>,
}

pub struct EreZisk {
    elf: Vec<u8>,
    resource: ProverResourceType,
}

impl EreZisk {
    pub fn new(elf: Vec<u8>, resource: ProverResourceType) -> Self {
        Self { elf, resource }
    }
}

impl EreZisk {}

impl zkVM for EreZisk {
    fn execute(&self, input: &Input) -> Result<ProgramExecutionReport, zkVMError> {
        // Write ELF and serialized input to file.

        let input_bytes = input
            .iter()
            .try_fold(Vec::new(), |mut acc, item| {
                acc.extend(item.as_bytes().map_err(ExecuteError::SerializeInput)?);
                Ok(acc)
            })
            .map_err(ZiskError::Execute)?;

        let mut tempdir =
            ZiskTempDir::new(false).map_err(|e| ZiskError::Execute(ExecuteError::TempDir(e)))?;
        tempdir
            .write_elf(&self.elf)
            .map_err(|e| ZiskError::Execute(ExecuteError::TempDir(e)))?;
        tempdir
            .write_input(&input_bytes)
            .map_err(|e| ZiskError::Execute(ExecuteError::TempDir(e)))?;

        // Execute.

        let start = time::Instant::now();
        let output = Command::new("ziskemu")
            .arg("--elf")
            .arg(tempdir.elf_path())
            .arg("--inputs")
            .arg(tempdir.input_path())
            .arg("--stats") // NOTE: enable stats in order to get total steps.
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| ZiskError::Execute(ExecuteError::Ziskemu { source: e }))?;

        if !output.status.success() {
            return Err(ZiskError::Execute(ExecuteError::ZiskemuFailed {
                status: output.status,
            })
            .into());
        }
        let execution_duration = start.elapsed();

        // Extract cycle count from the stdout.

        let total_num_cycles = String::from_utf8_lossy(&output.stdout)
            .split_once("total steps = ")
            .and_then(|(_, stats)| {
                stats
                    .split_whitespace()
                    .next()
                    .and_then(|steps| steps.parse::<u64>().ok())
            })
            .ok_or(ZiskError::Execute(ExecuteError::TotalStepsNotFound))?;

        Ok(ProgramExecutionReport {
            total_num_cycles,
            execution_duration,
            ..Default::default()
        })
    }

    fn prove(&self, input: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError> {
        // Write ELF and serialized input to file.

        let input_bytes = input
            .iter()
            .try_fold(Vec::new(), |mut acc, item| {
                acc.extend(item.as_bytes().map_err(ProveError::SerializeInput)?);
                Ok(acc)
            })
            .map_err(ZiskError::Prove)?;

        let mut tempdir =
            ZiskTempDir::new(true).map_err(|e| ZiskError::Prove(ProveError::TempDir(e)))?;
        tempdir
            .write_elf(&self.elf)
            .map_err(|e| ZiskError::Prove(ProveError::TempDir(e)))?;
        tempdir
            .write_input(&input_bytes)
            .map_err(|e| ZiskError::Prove(ProveError::TempDir(e)))?;

        // Setup ROM.

        if !is_bin_exists(&self.elf, tempdir.elf_path()) {
            let status = Command::new("cargo-zisk")
                .arg("rom-setup")
                .arg("--elf")
                .arg(tempdir.elf_path())
                .arg("--zisk-path")
                .arg(tempdir.zisk_dir_path())
                .status()
                .map_err(|e| ZiskError::Prove(ProveError::CargoZiskRomSetup { source: e }))?;

            if !status.success() {
                return Err(
                    ZiskError::Prove(ProveError::CargoZiskRomSetupFailed { status }).into(),
                );
            }
        }

        // Prove.

        // TODO: Use `mpirun --np {num_processes} cargo-zisk prove ...` to
        //       utilize multiple CPUs, probably need the `ProverResourceType`
        //       to specify the number of available CPUs.

        let start = time::Instant::now();
        match self.resource {
            ProverResourceType::Cpu => {
                let status = Command::new("cargo-zisk")
                    .arg("prove")
                    .arg("--elf")
                    .arg(tempdir.elf_path())
                    .arg("--input")
                    .arg(tempdir.input_path())
                    .arg("--output-dir")
                    .arg(tempdir.output_dir_path())
                    .args([
                        "--aggregation",
                        "--verify-proofs",
                        "--save-proofs",
                        // Uncomment this when in memory constrained environment.
                        // "--unlock-mapped-memory",
                    ])
                    .status()
                    .map_err(|e| ZiskError::Prove(ProveError::CargoZiskProve { source: e }))?;

                if !status.success() {
                    return Err(
                        ZiskError::Prove(ProveError::CargoZiskProveFailed { status }).into(),
                    );
                }
            }
            ProverResourceType::Gpu => {
                // TODO: Set env `CUDA_VISIBLE_DEVICES = {0..num_devices}` to
                //       control how many GPUs to use, probably need the `ProverResourceType`
                //       to specify the number of available GPUs.
                let witness_lib_path = dot_zisk_dir_path()
                    .join("bin")
                    .join("libzisk_witness_gpu.so");
                let status = Command::new("cargo-zisk-gpu")
                    .arg("prove")
                    .arg("--witness-lib")
                    .arg(witness_lib_path)
                    .arg("--elf")
                    .arg(tempdir.elf_path())
                    .arg("--input")
                    .arg(tempdir.input_path())
                    .arg("--output-dir")
                    .arg(tempdir.output_dir_path())
                    .args([
                        "--aggregation",
                        "--verify-proofs",
                        "--save-proofs",
                        "--preallocate",
                        // Uncomment this when in memory constrained environment.
                        // "--unlock-mapped-memory",
                    ])
                    .status()
                    .map_err(|e| ZiskError::Prove(ProveError::CargoZiskProve { source: e }))?;

                if !status.success() {
                    return Err(
                        ZiskError::Prove(ProveError::CargoZiskProveFailed { status }).into(),
                    );
                }
            }
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for ZisK. Use CPU or GPU resource type."
                );
            }
        }
        let proving_time = start.elapsed();

        // Read proof and public values.

        let proof_with_public_values = ZiskProofWithPublicValues {
            proof: tempdir
                .read_proof()
                .map_err(|e| ZiskError::Prove(ProveError::TempDir(e)))?,
            public_values: tempdir
                .read_public_values()
                .map_err(|e| ZiskError::Prove(ProveError::TempDir(e)))?,
        };
        let bytes = bincode::serialize(&proof_with_public_values)
            .map_err(|err| ZiskError::Prove(ProveError::Bincode(err)))?;

        Ok((bytes, ProgramProvingReport::new(proving_time)))
    }

    fn verify(&self, bytes: &[u8]) -> Result<(), zkVMError> {
        // Write proof and public values to file.

        let proof_with_public_values: ZiskProofWithPublicValues = bincode::deserialize(bytes)
            .map_err(|err| ZiskError::Verify(VerifyError::Bincode(err)))?;

        let mut tempdir =
            ZiskTempDir::new(false).map_err(|e| ZiskError::Verify(VerifyError::TempDir(e)))?;
        tempdir
            .write_proof(&proof_with_public_values.proof)
            .map_err(|e| ZiskError::Verify(VerifyError::TempDir(e)))?;
        tempdir
            .write_public_values(&proof_with_public_values.public_values)
            .map_err(|e| ZiskError::Verify(VerifyError::TempDir(e)))?;

        // Verify.

        let output = Command::new("cargo-zisk")
            .arg("verify")
            .arg("--proof")
            .arg(tempdir.proof_path())
            .arg("--public-inputs")
            .arg(tempdir.public_values_path())
            .output()
            .map_err(|e| ZiskError::Verify(VerifyError::CargoZiskVerify { source: e }))?;

        if !output.status.success() {
            return Err(ZiskError::Verify(VerifyError::InvalidProof(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
            .into());
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

fn dot_zisk_dir_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set")).join(".zisk")
}

/// Check if these files exists in `$HOME/.zisk/cache`:
///
/// - `{elf_file_stem}-{elf_hash}-mo.bin`
/// - `{elf_file_stem}-{elf_hash}-mt.bin`
/// - `{elf_file_stem}-{elf_hash}-rh.bin`
///
/// Which are generated by `cargo-zisk rom-setup ...`.
fn is_bin_exists(elf: &[u8], elf_path: impl AsRef<Path>) -> bool {
    let stem = elf_path
        .as_ref()
        .file_stem()
        .expect("ELF file has name")
        .to_str()
        .expect("ELF file name is valid UTF-8");
    let hash = blake3::hash(elf).to_hex().to_string();
    ["mo", "mt", "rh"].into_iter().all(|suffix| {
        fs::exists(
            dot_zisk_dir_path()
                .join("cache")
                .join(format!("{stem}-{hash}-{suffix}.bin")),
        )
        .ok()
            == Some(true)
    })
}

struct ZiskTempDir {
    tempdir: TempDir,
    elf_hash: Option<String>,
}

impl ZiskTempDir {
    /// Create temporary directories for:
    /// - `guest.elf` - ELF compiled from guest program.
    /// - `zisk/` - Directory for building process during `rom-setup`.
    /// - `input.bin` - Input of execution or proving.
    /// - `output/vadcop_final_proof.json` - Aggregated proof generated by proving.
    /// - `output/publics.json` - Public values generated by proving.
    ///
    /// Set `with_zisk_dir` only when `rom-setup` is to be used.
    fn new(with_zisk_dir: bool) -> io::Result<Self> {
        let tempdir = Self {
            tempdir: tempdir()?,
            elf_hash: None,
        };

        fs::create_dir(tempdir.output_dir_path())?;

        if with_zisk_dir {
            fs::create_dir_all(tempdir.zisk_dir_path())?;

            // Check the global zisk directory exists.
            let global_zisk_dir_path = dot_zisk_dir_path().join("zisk");
            if !global_zisk_dir_path.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "Global .zisk/zisk directory not found at: {}",
                        global_zisk_dir_path.display()
                    ),
                ));
            }

            // Symlink necessary files for `make` command of `cargo-zisk rom-setup`.
            // The `Makefile` can be found https://github.com/0xPolygonHermez/zisk/blob/main/emulator-asm/Makefile.
            symlink(
                dot_zisk_dir_path().join("bin"),
                tempdir.dot_zisk_dir_path().join("bin"),
            )?;
            let temp_zisk_dir_path = tempdir.zisk_dir_path();
            fs::create_dir_all(temp_zisk_dir_path.join("emulator-asm").join("build"))?;
            symlink(
                global_zisk_dir_path.join("emulator-asm").join("Makefile"),
                temp_zisk_dir_path.join("emulator-asm").join("Makefile"),
            )?;
            symlink(
                global_zisk_dir_path.join("emulator-asm").join("src"),
                temp_zisk_dir_path.join("emulator-asm").join("src"),
            )?;
            symlink(
                global_zisk_dir_path.join("lib-c"),
                temp_zisk_dir_path.join("lib-c"),
            )?;
        }

        Ok(tempdir)
    }

    fn write_elf(&mut self, elf: &[u8]) -> io::Result<()> {
        self.elf_hash = Some(blake3::hash(elf).to_hex().to_string());
        fs::write(self.elf_path(), elf)
    }

    fn write_input(&mut self, input: &[u8]) -> io::Result<()> {
        fs::File::create(self.input_path()).and_then(|mut file| file.write_all(input))
    }

    fn read_proof(&self) -> io::Result<Vec<u8>> {
        fs::read(self.proof_path())
    }

    fn write_proof(&mut self, proof: &[u8]) -> io::Result<()> {
        fs::File::create(self.proof_path()).and_then(|mut file| file.write_all(proof))
    }

    fn read_public_values(&self) -> io::Result<Vec<u8>> {
        fs::read(self.public_values_path())
    }

    fn write_public_values(&mut self, public_values: &[u8]) -> io::Result<()> {
        fs::File::create(self.public_values_path())
            .and_then(|mut file| file.write_all(public_values))
    }

    fn elf_path(&self) -> PathBuf {
        self.tempdir.path().join("guest.elf")
    }

    fn dot_zisk_dir_path(&self) -> PathBuf {
        self.tempdir.path().join(".zisk")
    }

    fn zisk_dir_path(&self) -> PathBuf {
        self.dot_zisk_dir_path().join("zisk")
    }

    fn input_path(&self) -> PathBuf {
        self.tempdir.path().join("input.bin")
    }

    fn output_dir_path(&self) -> PathBuf {
        self.tempdir.path().join("output")
    }

    fn public_values_path(&self) -> PathBuf {
        self.output_dir_path().join("publics.json")
    }

    fn proof_path(&self) -> PathBuf {
        self.output_dir_path().join("vadcop_final_proof.bin")
    }
}

#[cfg(test)]
mod execute_tests {
    use super::*;

    fn get_compiled_test_zisk_elf() -> Result<Vec<u8>, ZiskError> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV64_IMA_ZISK_ZKVM_ELF::compile(&test_guest_path, Path::new(""))
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zisk")
            .join("execute")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/zisk")
    }

    #[test]
    fn test_execute_zisk_dummy_input() {
        let elf_bytes = get_compiled_test_zisk_elf()
            .expect("Failed to compile test ZisK guest for execution test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreZisk::new(elf_bytes, ProverResourceType::Cpu);

        let result = zkvm.execute(&input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {e:?}");
        }
    }

    #[test]
    fn test_execute_zisk_no_input_for_guest_expecting_input() {
        let elf_bytes = get_compiled_test_zisk_elf()
            .expect("Failed to compile test ZisK guest for execution test");

        let empty_input = Input::new();

        let zkvm = EreZisk::new(elf_bytes, ProverResourceType::Cpu);
        assert!(zkvm.execute(&empty_input).is_err());
    }
}

#[cfg(test)]
mod prove_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_prove_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zisk")
            .join("prove")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/zisk")
    }

    fn get_compiled_test_zisk_elf_for_prove() -> Result<Vec<u8>, ZiskError> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV64_IMA_ZISK_ZKVM_ELF::compile(&test_guest_path, Path::new(""))
    }

    #[test]
    fn test_prove_zisk_dummy_input() {
        let elf_path = get_compiled_test_zisk_elf_for_prove()
            .expect("Failed to compile test ZisK guest for proving test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreZisk::new(elf_path, ProverResourceType::Cpu);

        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((prove_result, _)) => prove_result,
            Err(err) => {
                panic!("Proving error in test: {err:?}");
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        assert!(zkvm.verify(&proof_bytes).is_ok());

        let invalid_proof_bytes = {
            let mut invalid_proof: ZiskProofWithPublicValues =
                bincode::deserialize(&proof_bytes).unwrap();
            // alter the first digit of `evals[0][0]`
            invalid_proof.proof[40] = invalid_proof.proof[40].overflowing_add(1).0;
            bincode::serialize(&invalid_proof).unwrap()
        };
        assert!(zkvm.verify(&invalid_proof_bytes).is_err());

        // TODO: Check public inputs
    }

    #[test]
    fn test_prove_zisk_fails_on_bad_input_causing_execution_failure() {
        let elf_path = get_compiled_test_zisk_elf_for_prove()
            .expect("Failed to compile test ZisK guest for proving test");

        let empty_input = Input::new();

        let zkvm = EreZisk::new(elf_path, ProverResourceType::Cpu);
        assert!(zkvm.prove(&empty_input).is_err());
    }
}
