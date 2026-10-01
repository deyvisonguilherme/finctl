use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Erro de conexão com o banco de dados: {0}")]
    Connection(String),

    #[error("Erro ao executar query no banco de dados: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Erro ao aplicar migrations: {0}")]
    Migration(String),

    #[error("Registro não encontrado: {0}")]
    NotFound(String),

    #[error("Registro duplicado ou conflito de chave única: {0}")]
    UniqueViolation(String),

    #[error("Erro de conversão de dados: {0}")]
    Conversion(String),
}
