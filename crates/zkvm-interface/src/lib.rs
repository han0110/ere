use indexmap::IndexMap;
use serde::Serialize;
use std::{path::Path, time::Duration};

#[allow(non_camel_case_types)]
/// zkVM trait to abstract away the differences between each zkVM
pub trait zkVM {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Compiles the program and returns the `ELF` as bytes
    fn compile(path_to_program: &Path) -> Result<Vec<u8>, Self::Error>;

    /// Executes the given ELF binary with the inputs accumulated in the Input struct.
    fn execute(elf_bytes: &[u8], inputs: &Input) -> Result<ProgramExecutionReport, Self::Error>;

    /// Creates a proof for the given program
    fn prove(
        elf_bytes: &[u8],
        inputs: &Input,
    ) -> Result<(Vec<u8>, ProgramProvingReport), Self::Error>;

    /// Verifies a proof for the given program
    fn verify(elf_bytes: &[u8], proof: &[u8]) -> Result<(), Self::Error>;
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

/// Represents a builder for input data to be passed to a ZKVM guest program.
/// Values are serialized sequentially into an internal byte buffer.
#[derive(Debug, Clone, Default)] // Added Default for easy initialization
pub struct Input {
    // TODO: Succinct has Vec<Vec<u8> while R0 has Vec<u8>
    // TODO: Maybe change back to Vec<u8> with markers for perf
    pub data: Vec<Vec<u8>>,
}

impl Input {
    pub fn new() -> Self {
        Input { data: Vec::new() }
    }

    /// Serializes the given value using bincode and appends it to the internal data buffer.
    pub fn write<T: Serialize>(&mut self, value: &T) -> anyhow::Result<()> {
        // TODO: Remove anyhow::error for thiserror
        use anyhow::Context;

        let mut data = Vec::new();

        let _ = bincode::serialize_into(&mut data, value).with_context(|| {
            format!(
                "Failed to serialize and write value of type {}",
                std::any::type_name::<T>()
            )
        })?;

        self.data.push(data);

        Ok(())
    }
}
