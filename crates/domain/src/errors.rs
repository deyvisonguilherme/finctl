use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum DomainError {
    #[error("Valor monetário inválido: {0}")]
    InvalidMoney(String),

    #[error("Regra de negócio violada: {0}")]
    Validation(String),

    #[error("Entidade não encontrada: {0}")]
    NotFound(String),
}
