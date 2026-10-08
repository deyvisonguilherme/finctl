pub mod cli;
pub mod commands;
pub mod format;

pub use cli::*;
pub use commands::account::handle_account_command;
pub use commands::audit::handle_audit_command;
pub use commands::backup_cmd::handle_backup_command;
pub use commands::balance::handle_balance_command;
pub use commands::budget::handle_budget_command;
pub use commands::card::handle_card_command;
pub use commands::category::handle_category_command;
pub use commands::completions::handle_completions_command;
pub use commands::expense::handle_expense_command;
pub use commands::export::handle_export_command;
pub use commands::import_cmd::handle_import_command;
pub use commands::income::handle_income_command;
pub use commands::init::handle_init_command;
pub use commands::man::handle_man_command;
pub use commands::purge::handle_purge_command;
pub use commands::reconcile::handle_reconcile_command;
pub use commands::recurring::handle_recurring_command;
pub use commands::report::handle_report_command;
pub use commands::restore_cmd::handle_restore_command;
pub use commands::tag::handle_tag_command;
pub use commands::transfer::handle_transfer_command;
pub use commands::tx::handle_tx_command;

use clap::{CommandFactory, Parser};
use domain::UserId;
use std::str::FromStr;

pub fn build_cli() -> clap::Command {
    Cli::command()
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

pub async fn run() -> Result<(), (String, u8)> {
    dotenvy::dotenv().ok();
    let _ = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();

    match cli.command {
        None => {
            println!("finctl {}", env!("CARGO_PKG_VERSION"));
            println!("Use `finctl --help` para ver os comandos disponíveis.");
            Ok(())
        }
        Some(Commands::Completions(args)) => {
            handle_completions_command(args, Cli::command());
            Ok(())
        }
        Some(Commands::Man(args)) => {
            handle_man_command(args, Cli::command())
                .map_err(|e| (format!("Erro ao gerar man page: {e}"), 1))?;
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
            DbCommands::Backup(args) => {
                let db_url = get_database_url()?;
                let pool = storage::create_pool(&db_url)
                    .await
                    .map_err(|e| (e.to_string(), 2))?;
                handle_backup_command(args, &pool, &db_url).await
            }
            DbCommands::Restore(args) => {
                let db_url = get_database_url()?;
                let pool = storage::create_pool(&db_url)
                    .await
                    .map_err(|e| (e.to_string(), 2))?;
                handle_restore_command(args, &pool, &db_url).await
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
        Some(Commands::Transfer { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_transfer_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Card { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_card_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Tx { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_tx_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Budget { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_budget_command(&pool, user_id, subcommand).await
        }
        Some(Commands::Recurring { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_recurring_command(&pool, user_id, subcommand).await
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
        Some(Commands::Reconcile(args)) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_reconcile_command(&pool, user_id, args).await
        }
        Some(Commands::Purge(args)) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_purge_command(args, &pool, user_id).await
        }
        Some(Commands::Tag { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            let user_id = get_current_user_id();
            handle_tag_command(subcommand, &pool, user_id).await
        }
        Some(Commands::Audit { subcommand }) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            handle_audit_command(subcommand, &pool).await
        }
        Some(Commands::Backup(args)) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            handle_backup_command(args, &pool, &db_url).await
        }
        Some(Commands::Restore(args)) => {
            let db_url = get_database_url()?;
            let pool = storage::create_pool(&db_url)
                .await
                .map_err(|e| (e.to_string(), 2))?;
            handle_restore_command(args, &pool, &db_url).await
        }
    }
}
