use compile::compile_risczero_program;
use zkvm_interface::Compiler;

mod compile;

mod error;
use error::RiscZeroError;

#[allow(non_camel_case_types)]
pub struct RV32_IM_RISCZERO_ZKVM_ELF;

impl Compiler for RV32_IM_RISCZERO_ZKVM_ELF {
    type Error = RiscZeroError;

    type Program = Vec<u8>;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        compile_risczero_program(path_to_program).map_err(RiscZeroError::from)
    }
}
