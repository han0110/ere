#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use compile::compile_sp1_program;
use sp1_sdk::{Prover, ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use thiserror::Error;
use tracing::info;
use zkvm_interface::{Compiler, ProgramExecutionReport, ProgramProvingReport, zkVM};

mod compile;

// Represents Ere compliant API for SP1
pub struct EreSP1;

#[derive(Debug, thiserror::Error)]
pub enum SP1Error {
    #[error(transparent)]
    CompileError(#[from] compile::CompileError),

    #[error(transparent)]
    Execute(#[from] ExecuteError),

    #[error(transparent)]
    Prove(#[from] ProveError),

    #[error(transparent)]
    Verify(#[from] VerifyError),
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("SP1 execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("SP1 SDK proving failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Deserialising proof failed: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("SP1 SDK verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Compiler for EreSP1 {
    type Error = SP1Error;

    type Program = Vec<u8>;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        compile_sp1_program(path_to_program).map_err(SP1Error::from)
    }
}

impl zkVM<EreSP1> for EreSP1 {
    type Error = SP1Error;

    fn execute(
        program_bytes: &<Self as Compiler>::Program,
        inputs: &zkvm_interface::Input,
    ) -> Result<zkvm_interface::ProgramExecutionReport, Self::Error> {
        // TODO: This is expensive, should move it out and make the struct stateful
        let client = ProverClient::builder().cpu().build();

        let mut stdin = SP1Stdin::new();
        for input in inputs.chunked_iter() {
            stdin.write_slice(input);
        }

        let (_, exec_report) = client
            .execute(&program_bytes, &stdin)
            .run()
            .map_err(|e| ExecuteError::Client(e.into()))?;

        Ok(ProgramExecutionReport::new(
            exec_report.total_instruction_count(),
        ))
    }

    fn prove(
        program_bytes: &<Self as Compiler>::Program,
        inputs: &zkvm_interface::Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), Self::Error> {
        info!("Generating proof…");

        // TODO: This is expensive, should move it out and make the struct stateful
        let client = ProverClient::builder().cpu().build();
        // TODO: This can also be cached
        let (pk, _vk) = client.setup(&program_bytes);

        let mut stdin = SP1Stdin::new();
        for input in inputs.chunked_iter() {
            stdin.write_slice(input);
        }

        let start = std::time::Instant::now();
        let proof_with_inputs = client
            .prove(&pk, &stdin)
            .core()
            .run()
            .map_err(|e| ProveError::Client(e.into()))?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof_with_inputs)
            .map_err(|err| SP1Error::Verify(VerifyError::Bincode(err)))?;

        Ok((bytes, ProgramProvingReport::new(proving_time)))
    }

    fn verify(
        program_bytes: &<Self as Compiler>::Program,
        proof: &[u8],
    ) -> Result<(), Self::Error> {
        info!("Verifying proof…");

        let client = ProverClient::from_env();
        let (_pk, vk) = client.setup(&program_bytes);

        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| SP1Error::Verify(VerifyError::Bincode(err)))?;

        client
            .verify(&proof, &vk)
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
        EreSP1::compile(&test_guest_path)
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

        let result = EreSP1::execute(&elf_bytes, &input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {:?}", e);
        }
    }

    #[test]
    fn test_execute_sp1_no_input_for_guest_expecting_input() {
        let elf_bytes = get_compiled_test_sp1_elf()
            .expect("Failed to compile test SP1 guest for execution test");

        let empty_input = Input::new();

        let result = EreSP1::execute(&elf_bytes, &empty_input);

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
        EreSP1::compile(&test_guest_path)
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

        let proof_bytes = match EreSP1::prove(&elf_bytes, &input_builder) {
            Ok((prove_result, _)) => prove_result,
            Err(err) => {
                panic!("Proving error in test: {:?}", err);
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = EreSP1::verify(&elf_bytes, &proof_bytes).is_ok();
        assert!(verify_results);

        // TODO: Check public inputs
    }

    #[test]
    #[should_panic]
    fn test_prove_sp1_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_sp1_elf_for_prove()
            .expect("Failed to compile test SP1 guest for proving test");

        let empty_input = Input::new();

        let prove_result = EreSP1::prove(&elf_bytes, &empty_input);
        assert!(prove_result.is_err())
    }
}
