use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "finctl",
    version,
    about = "Sistema de controle financeiro pessoal"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Operações de banco de dados
    Db {
        #[command(subcommand)]
        subcommand: DbCommands,
    },
}

#[derive(Subcommand, Debug)]
enum DbCommands {
    /// Testa a conexão com o banco de dados
    Ping,
    /// Aplica as migrações pendentes no banco de dados
    Migrate,
}

pub fn get_database_url() -> Result<String, (String, u8)> {
    std::env::var("DATABASE_URL").map_err(|_| {
        (
            "Erro: variável de ambiente DATABASE_URL não configurada. Defina no arquivo .env ou no ambiente.".to_string(),
            2,
        )
    })
}

async fn run() -> Result<(), (String, u8)> {
    dotenvy::dotenv().ok();
    let _ = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();

    match cli.command {
        None => {
            println!("finctl {}", env!("CARGO_PKG_VERSION"));
            println!("Use `finctl --help` para ver os comandos disponíveis.");
            Ok(())
        }
        Some(Commands::Db { subcommand }) => match subcommand {
            DbCommands::Ping => {
                let db_url = get_database_url()?;
                let pool = storage::create_pool(&db_url)
                    .await
                    .map_err(|e| (e.to_string(), 2))?;
                storage::ping(&pool).await.map_err(|e| (e.to_string(), 2))?;
                println!("Conexão com o banco de dados realizada com sucesso!");
                Ok(())
            }
            DbCommands::Migrate => {
                let db_url = get_database_url()?;
                let pool = storage::create_pool(&db_url)
                    .await
                    .map_err(|e| (e.to_string(), 2))?;
                storage::run_migrations(&pool)
                    .await
                    .map_err(|e| (e.to_string(), 2))?;
                println!("Migrações aplicadas com sucesso!");
                Ok(())
            }
        },
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err((msg, code)) = run().await {
        eprintln!("{msg}");
        ExitCode::from(code)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_database_url_returns_code_2() {
        std::env::remove_var("DATABASE_URL");
        let err = get_database_url().unwrap_err();
        assert_eq!(err.1, 2);
        assert!(err.0.contains("DATABASE_URL"));
    }
}
