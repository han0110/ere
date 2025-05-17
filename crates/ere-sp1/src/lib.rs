#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use compile::compile_sp1_program;
use sp1_sdk::{
    CpuProver, Prover, ProverClient, SP1ProofWithPublicValues, SP1ProvingKey, SP1Stdin,
    SP1VerifyingKey,
};
use tracing::info;
use zkvm_interface::{Compiler, ProgramExecutionReport, ProgramProvingReport, zkVM};

mod compile;

mod error;
use error::{ExecuteError, ProveError, SP1Error, VerifyError};

#[allow(non_camel_case_types)]
pub struct RV32_IM_SUCCINCT_ZKVM_ELF;
pub struct EreSP1 {
    program: <RV32_IM_SUCCINCT_ZKVM_ELF as Compiler>::Program,
    /// Proving key
    pk: SP1ProvingKey,
    /// Verification key
    vk: SP1VerifyingKey,
    /// Proof and Verification orchestrator
    client: CpuProver,
}

impl Compiler for RV32_IM_SUCCINCT_ZKVM_ELF {
    type Error = SP1Error;

    type Program = Vec<u8>;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        compile_sp1_program(path_to_program).map_err(SP1Error::from)
    }
}

impl zkVM<RV32_IM_SUCCINCT_ZKVM_ELF> for EreSP1 {
    type Error = SP1Error;

    fn new(program: <RV32_IM_SUCCINCT_ZKVM_ELF as Compiler>::Program) -> Self {
        let client = ProverClient::builder().cpu().build();
        let (pk, vk) = client.setup(&program);

        Self {
            program,
            client,
            pk,
            vk,
        }
    }

    fn execute(
        &self,
        inputs: &zkvm_interface::Input,
    ) -> Result<zkvm_interface::ProgramExecutionReport, Self::Error> {
        let mut stdin = SP1Stdin::new();
        for input in inputs.chunked_iter() {
            stdin.write_slice(input);
        }

        let (_, exec_report) = self
            .client
            .execute(&self.program, &stdin)
            .run()
            .map_err(|e| ExecuteError::Client(e.into()))?;

        let total_num_cycles = exec_report.total_instruction_count();
        let region_cycles: indexmap::IndexMap<_, _> =
            exec_report.cycle_tracker.into_iter().collect();

        let mut ere_report = ProgramExecutionReport::new(total_num_cycles);
        ere_report.region_cycles = region_cycles;

        Ok(ere_report)
    }

    fn prove(
        &self,
        inputs: &zkvm_interface::Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), Self::Error> {
        info!("Generating proof…");

        let mut stdin = SP1Stdin::new();
        for input in inputs.chunked_iter() {
            stdin.write_slice(input);
        }

        let start = std::time::Instant::now();
        let proof_with_inputs = self
            .client
            .prove(&self.pk, &stdin)
            .core()
            .run()
            .map_err(|e| ProveError::Client(e.into()))?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof_with_inputs)
            .map_err(|err| SP1Error::Prove(ProveError::Bincode(err)))?;

        Ok((bytes, ProgramProvingReport::new(proving_time)))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), Self::Error> {
        info!("Verifying proof…");

        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| SP1Error::Verify(VerifyError::Bincode(err)))?;

        self.client
            .verify(&proof, &self.vk)
            .map_err(|e| SP1Error::Verify(VerifyError::Client(e.into())))
    }
}

#[cfg(test)]
mod execute_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_compiled_test_sp1_elf() -> Result<Vec<u8>, SP1Error> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV32_IM_SUCCINCT_ZKVM_ELF::compile(&test_guest_path)
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("sp1")
            .join("execute")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/sp1")
    }

    #[test]
    fn test_execute_sp1_dummy_input() {
        let elf_bytes = get_compiled_test_sp1_elf()
            .expect("Failed to compile test SP1 guest for execution test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(&n).unwrap();
        input_builder.write(&a).unwrap();

        let zkvm = EreSP1::new(elf_bytes);

        let result = zkvm.execute(&input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {:?}", e);
        }
    }

    #[test]
    fn test_execute_sp1_no_input_for_guest_expecting_input() {
        let elf_bytes = get_compiled_test_sp1_elf()
            .expect("Failed to compile test SP1 guest for execution test");

        let empty_input = Input::new();

        let zkvm = EreSP1::new(elf_bytes);
        let result = zkvm.execute(&empty_input);

        assert!(
            result.is_err(),
            "execute should fail if guest expects input but none is provided."
        );
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
            .join("sp1")
            .join("prove")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/sp1")
    }

    fn get_compiled_test_sp1_elf_for_prove() -> Result<Vec<u8>, SP1Error> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV32_IM_SUCCINCT_ZKVM_ELF::compile(&test_guest_path)
    }

    #[test]
    fn test_prove_sp1_dummy_input() {
        let elf_bytes = get_compiled_test_sp1_elf_for_prove()
            .expect("Failed to compile test SP1 guest for proving test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(&n).unwrap();
        input_builder.write(&a).unwrap();

        let zkvm = EreSP1::new(elf_bytes);

        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((prove_result, _)) => prove_result,
            Err(err) => {
                panic!("Proving error in test: {:?}", err);
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = zkvm.verify(&proof_bytes).is_ok();
        assert!(verify_results);

        // TODO: Check public inputs
    }

    #[test]
    #[should_panic]
    fn test_prove_sp1_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_sp1_elf_for_prove()
            .expect("Failed to compile test SP1 guest for proving test");

        let empty_input = Input::new();

        let zkvm = EreSP1::new(elf_bytes);
        let prove_result = zkvm.prove(&empty_input);
        assert!(prove_result.is_err())
    }
}
