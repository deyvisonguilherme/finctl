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

/// Detecta o ator corrente com base nas variáveis de ambiente ou usuário do SO via whoami
pub fn detect_current_actor() -> String {
    std::env::var("FINCTL_ACTOR")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| whoami::username())
}

/// Configura a variável local finctl.actor na conexão/transação via set_config local
pub async fn set_actor<'a, E>(executor: E, actor: &str) -> Result<(), StorageError>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    sqlx::query("SELECT set_config('finctl.actor', $1, true)")
        .bind(actor)
        .execute(executor)
        .await
        .map_err(StorageError::Database)?;
    Ok(())
}

/// Inicia uma transação configurando a variável local finctl.actor com o ator informado
pub async fn begin_with_actor<'a>(
    pool: &'a PgPool,
    actor: &str,
) -> Result<sqlx::Transaction<'a, sqlx::Postgres>, StorageError> {
    let mut tx = pool.begin().await.map_err(StorageError::Database)?;
    set_actor(&mut *tx, actor).await?;
    Ok(tx)
}

/// Inicia uma transação configurando a variável local finctl.actor com o usuário detectado do SO
pub async fn begin_tx<'a>(
    pool: &'a PgPool,
) -> Result<sqlx::Transaction<'a, sqlx::Postgres>, StorageError> {
    let actor = detect_current_actor();
    begin_with_actor(pool, &actor).await
}
