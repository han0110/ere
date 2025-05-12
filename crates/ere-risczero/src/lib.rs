use thiserror::Error;
use zkvm_interface::Compiler;

mod compile;

#[allow(non_camel_case_types)]
pub struct RV32_IM_RISCZERO_ZKVM_ELF;

#[derive(Debug, Error)]
pub enum RiscZeroError {}

impl Compiler for RV32_IM_RISCZERO_ZKVM_ELF {
    type Error = RiscZeroError;

    type Program = Vec<u8>;

    fn compile(_path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        todo!()
    }
}
