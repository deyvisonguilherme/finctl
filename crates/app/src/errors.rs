use domain::DomainError;
use storage::StorageError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    Domain(#[from] DomainError),

    #[error("{0}")]
    Storage(#[from] StorageError),

    #[error("Validação: {0}")]
    Validation(String),

    #[error("Não encontrado: {0}")]
    NotFound(String),
}
