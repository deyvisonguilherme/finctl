pub mod commands;
pub mod format;

use clap::{Parser, Subcommand};
use commands::account::{handle_account_command, AccountCommands};
use commands::balance::{handle_balance_command, BalanceArgs};
use commands::category::{handle_category_command, CategoryCommands};
use commands::expense::{handle_expense_command, ExpenseCommands};
use commands::export::{handle_export_command, ExportCommands};
use commands::import_cmd::{handle_import_command, ImportCommands};
use commands::income::{handle_income_command, IncomeCommands};
use commands::init::{handle_init_command, InitArgs};
use commands::report::{handle_report_command, ReportCommands};
use commands::tx::{handle_tx_command, TxCommands};
use domain::UserId;
use std::process::ExitCode;
use std::str::FromStr;

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
    /// Inicializa o banco de dados e cria as categorias padrão
    Init(InitArgs),

    /// Operações de banco de dados
    Db {
        #[command(subcommand)]
        subcommand: DbCommands,
    },

    /// Gerenciamento de contas bancárias e carteiras
    Account {
        #[command(subcommand)]
        subcommand: AccountCommands,
    },

    /// Gerenciamento de categorias de receitas e despesas
    Category {
        #[command(subcommand)]
        subcommand: CategoryCommands,
    },

    /// Registra uma receita
    Income {
        #[command(subcommand)]
        subcommand: IncomeCommands,
    },

    /// Registra uma despesa
    Expense {
        #[command(subcommand)]
        subcommand: ExpenseCommands,
    },

    /// Consulta e gerenciamento de transações/lançamentos
    Tx {
        #[command(subcommand)]
        subcommand: TxCommands,
    },

    /// Consulta de saldos consolidados por conta e total geral
    Balance(BalanceArgs),

    /// Relatórios financeiros e comparativos
    Report {
        #[command(subcommand)]
        subcommand: ReportCommands,
    },

    /// Exportação de dados para arquivos (CSV, JSON)
    Export {
        #[command(subcommand)]
        subcommand: ExportCommands,
    },

    /// Importação de dados a partir de arquivos externos
    Import {
        #[command(subcommand)]
        subcommand: ImportCommands,
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

pub fn get_current_user_id() -> UserId {
    let id_str = std::env::var("FINCTL_USER_ID")
        .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000001".to_string());
    UserId::from_str(&id_str)
        .unwrap_or_else(|_| UserId::from_str("00000000-0000-0000-0000-000000000001").unwrap())
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
        Some(Commands::Init(args)) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_init_command(args, &pool, user_id).await
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
        Some(Commands::Account { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_account_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Category { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_category_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Income { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_income_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Expense { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_expense_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Tx { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_tx_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Balance(args)) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_balance_command(args, &pool, user_id).await
        }
        Some(Commands::Report { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_report_command(&pool, user_id, subcommand).await
        }
        Some(Commands::Export { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_export_command(&pool, user_id, subcommand).await
        }
        Some(Commands::Import { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_import_command(&pool, user_id, subcommand).await
        }
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
