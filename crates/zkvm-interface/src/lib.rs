use indexmap::IndexMap;
use std::{path::Path, time::Duration};

mod input;
pub use input::Input;

#[allow(non_camel_case_types)]
/// Compiler trait for compiling programs into an opaque sequence of bytes.
pub trait Compiler {
    type Error: std::error::Error + Send + Sync + 'static;
    type Program: Clone + Send + Sync;

    /// Compiles the program and returns the program
    fn compile(path_to_program: &Path) -> Result<Self::Program, Self::Error>;
}

#[allow(non_camel_case_types)]
/// zkVM trait to abstract away the differences between each zkVM
pub trait zkVM<C: Compiler> {
    type Error: std::error::Error + Send + Sync + 'static;

    fn new(program_bytes: C::Program) -> Self;

    /// Executes the given program with the inputs accumulated in the Input struct.
    /// For RISCV programs, `program_bytes` will be the ELF binary
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, Self::Error>;

    /// Creates a proof for a given program
    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), Self::Error>;

    /// Verifies a proof for the given program
    /// TODO: Pass public inputs too and check that they match if they come with the
    /// TODO: proof, or append them if they do not.
    /// TODO: We can also just have this return the public inputs, but then the user needs
    /// TODO: ensure they check it for correct #[must_use]
    fn verify(&self, proof: &[u8]) -> Result<(), Self::Error>;
}

/// ProgramExecutionReport produces information about a particular program
/// execution.
#[derive(Debug, Clone, Default)]
pub struct ProgramExecutionReport {
    /// Total number of cycles for the entire workload execution.
    pub total_num_cycles: u64,
    /// Region-specific cycles, mapping region names (e.g., "setup", "compute") to their cycle counts.
    pub region_cycles: IndexMap<String, u64>,
}

impl ProgramExecutionReport {
    pub fn new(total_num_cycles: u64) -> Self {
        ProgramExecutionReport {
            total_num_cycles,
            region_cycles: Default::default(),
        }
    }
    pub fn insert_region(&mut self, region_name: String, num_cycles: u64) {
        self.region_cycles.insert(region_name, num_cycles);
    }
}

/// ProgramProvingReport produces information about proving a particular
/// program's instance.
#[derive(Debug, Clone, Default)]
pub struct ProgramProvingReport {
    pub proving_time: Duration,
}
impl ProgramProvingReport {
    pub fn new(proving_time: Duration) -> Self {
        Self { proving_time }
    }
}
