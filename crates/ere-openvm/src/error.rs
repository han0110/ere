use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpenVMError {
    #[error(transparent)]
    Compile(#[from] CompileError),

    #[error(transparent)]
    Verify(#[from] VerifyError),
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("OpenVM execution failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("OpenVM verification failed: {0}")]
    Client(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}
