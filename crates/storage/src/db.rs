use crate::errors::StorageError;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub async fn create_pool(database_url: &str) -> Result<PgPool, StorageError> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await
        .map_err(|e| {
            StorageError::Connection(format!("Não foi possível conectar ao Postgres: {e}"))
        })
}

pub async fn ping(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::query("SELECT 1").execute(pool).await.map_err(|e| {
        StorageError::Connection(format!("Falha no teste de ping com o banco: {e}"))
    })?;
    Ok(())
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| StorageError::Migration(format!("Falha ao aplicar migrations: {e}")))?;
    Ok(())
}
