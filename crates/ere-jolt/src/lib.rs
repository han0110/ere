use jolt_core::host::Program;
use jolt_methods::{preprocess_prover, preprocess_verifier, prove_generic, verify_generic};
use jolt_sdk::host::DEFAULT_TARGET_DIR;
use utils::{
    deserialize_public_input_with_proof, package_name_from_manifest,
    serialize_public_input_with_proof,
};
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, ProverResourceType, zkVM,
};

mod jolt_methods;
mod utils;

#[allow(non_camel_case_types)]
pub struct JOLT_TARGET;

#[derive(Debug, thiserror::Error)]
pub enum JoltError {
    #[error("Proof verification failed")]
    ProofVerificationFailed,
}

impl Compiler for JOLT_TARGET {
    type Error = JoltError;

    type Program = Program;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        let manifest_path = path_to_program.to_path_buf().join("Cargo.toml");
        let package_name = package_name_from_manifest(&manifest_path).unwrap();
        let mut program = Program::new(&package_name);
        program.set_std(true);
        program.set_manifest_path(manifest_path);

        // TODO: Note that if this fails, it will panic which is why it doesn't return a Result.
        program.build(DEFAULT_TARGET_DIR);

        Ok(program)
    }
}

pub struct EreJolt {
    program: <JOLT_TARGET as Compiler>::Program,
}

impl EreJolt {
    fn new(
        program: <JOLT_TARGET as Compiler>::Program,
        _resource_type: ProverResourceType,
    ) -> Self {
        EreJolt { program: program }
    }
}
impl zkVM for EreJolt {
    type Error = JoltError;

    fn execute(
        &self,
        inputs: &zkvm_interface::Input,
    ) -> Result<zkvm_interface::ProgramExecutionReport, Self::Error> {
        // TODO: check ProgramSummary
        let summary = self
            .program
            .clone()
            .trace_analyze::<jolt::F>(inputs.bytes());
        let trace_len = summary.trace_len();

        Ok(ProgramExecutionReport::new(trace_len as u64))
    }

    fn prove(
        &self,
        inputs: &zkvm_interface::Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), Self::Error> {
        // TODO: make this stateful and do in setup since its expensive and should be done once per program;
        let preprocessed_key = preprocess_prover(&self.program);

        let now = std::time::Instant::now();
        let (output_bytes, proof) = prove_generic(&self.program, preprocessed_key, inputs);
        let elapsed = now.elapsed();

        let proof_with_public_inputs =
            serialize_public_input_with_proof(&output_bytes, &proof).unwrap();

        Ok((proof_with_public_inputs, ProgramProvingReport::new(elapsed)))
    }

    fn verify(&self, proof_with_public_inputs: &[u8]) -> Result<(), Self::Error> {
        let preprocessed_verifier = preprocess_verifier(&self.program);
        let (public_inputs, proof) =
            deserialize_public_input_with_proof(proof_with_public_inputs).unwrap();

        let mut outputs = Input::new();
        assert!(public_inputs.is_empty());
        outputs.write(&public_inputs).unwrap();

        // TODO: I don't think we should require the inputs when verifying
        let inputs = Input::new();

        let valid = verify_generic(proof, inputs, outputs, preprocessed_verifier);
        if valid {
            Ok(())
        } else {
            Err(JoltError::ProofVerificationFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreJolt, JOLT_TARGET};
    use std::path::PathBuf;
    use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("jolt")
            .join("compile")
            .join("basic")
            .join("guest")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/jolt")
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program = JOLT_TARGET::compile(&test_guest_path).unwrap();
        assert!(program.elf.is_some(), "elf has not been compiled");
    }

    #[test]
    fn test_execute() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program = JOLT_TARGET::compile(&test_guest_path).unwrap();
        let mut inputs = Input::new();
        inputs.write(&(1 as u32)).unwrap();

        let zkvm = EreJolt::new(program, ProverResourceType::Cpu);
        let _execution = zkvm.execute(&inputs).unwrap();
    }
    // #[test]
    // fn test_prove_verify() {
    //     let test_guest_path = get_compile_test_guest_program_path();
    //     let program = JOLT_TARGET::compile(&test_guest_path).unwrap();

    //     // TODO: I don't think we should require the inputs when verifying
    //     let inputs = Input::new();

    //     let (proof, _) = EreJolt::prove(&program, &inputs).unwrap();
    //     EreJolt::verify(&program, &proof).unwrap();
    // }
}
