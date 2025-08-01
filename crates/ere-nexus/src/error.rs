use std::path::PathBuf;
use thiserror::Error;
use zkvm_interface::zkVMError;

impl From<NexusError> for zkVMError {
    fn from(value: NexusError) -> Self {
        zkVMError::Other(Box::new(value))
    }
}

#[derive(Debug, Error)]
pub enum NexusError {
    #[error(transparent)]
    Compile(#[from] CompileError),

    #[error(transparent)]
    Prove(#[from] ProveError),

    #[error(transparent)]
    Verify(#[from] VerifyError),

    /// Guest program directory does not exist.
    #[error("guest program directory not found: {0}")]
    PathNotFound(PathBuf),

    /// Expected ELF file was not produced.
    #[error("ELF file not found at {0}")]
    ElfNotFound(PathBuf),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("nexus execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum ProveError {
    #[error("nexus execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Serialising proof with `bincode` failed: {0}")]
    Bincode(#[from] bincode::Error),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("nexus verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Deserialising proof failed: {0}")]
    Bincode(#[from] bincode::Error),
}
