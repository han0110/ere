#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use compile::compile_sp1_program;
use zkvm_interface::Compiler;

mod compile;

// Represents Ere compliant API for SP1
pub struct EreSP1;

#[derive(Debug, thiserror::Error)]
pub enum SP1Error {
    #[error(transparent)]
    CompileError(#[from] compile::CompileError),
}

impl Compiler for EreSP1 {
    type Error = SP1Error;

    type Program = Vec<u8>;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        compile_sp1_program(path_to_program).map_err(SP1Error::from)
    }
}
