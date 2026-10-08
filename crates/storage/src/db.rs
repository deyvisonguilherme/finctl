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

#[derive(Debug, Clone)]
pub struct DatabaseConnectionInfo {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub database: String,
}

impl DatabaseConnectionInfo {
    pub fn parse(database_url: &str) -> Result<Self, StorageError> {
        let parsed_url = url::Url::parse(database_url)
            .map_err(|e| StorageError::Connection(format!("DATABASE_URL inválida: {e}")))?;

        let host = parsed_url.host_str().unwrap_or("localhost").to_string();
        let port = parsed_url.port().unwrap_or(5432);
        let username = parsed_url.username().to_string();
        let password = parsed_url.password().map(|p| p.to_string());
        let path = parsed_url.path().trim_start_matches('/');
        let database = if path.is_empty() {
            "postgres".to_string()
        } else {
            path.to_string()
        };

        Ok(Self {
            host,
            port,
            username,
            password,
            database,
        })
    }

    /// Retorna a DATABASE_URL apontando para um banco específico com as mesmas credenciais
    pub fn with_database(&self, new_db: &str) -> String {
        let auth = match &self.password {
            Some(pwd) => format!("{}:{}@", self.username, pwd),
            None => format!("{}@", self.username),
        };
        format!("postgres://{}{}:{}/{}", auth, self.host, self.port, new_db)
    }
}

/// Cria um novo banco de dados se não existir, utilizando o pool da conexão atual
pub async fn create_database_if_not_exists(
    pool: &PgPool,
    new_db_name: &str,
) -> Result<(), StorageError> {
    if !new_db_name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(StorageError::Connection(format!(
            "Nome de banco de dados inválido: '{new_db_name}'"
        )));
    }

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(new_db_name)
            .fetch_one(pool)
            .await
            .map_err(StorageError::Database)?;

    if !exists {
        let sql = format!("CREATE DATABASE \"{new_db_name}\"");
        sqlx::query(&sql)
            .execute(pool)
            .await
            .map_err(StorageError::Database)?;
    }

    Ok(())
}
