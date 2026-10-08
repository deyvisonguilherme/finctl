use app::{BackupService, RestoreInput};
use dialoguer::Input;
use sqlx::PgPool;
use std::io::IsTerminal;
use storage::DatabaseConnectionInfo;

pub use crate::cli::RestoreArgs;

pub async fn handle_restore_command(
    args: RestoreArgs,
    pool: &PgPool,
    database_url: &str,
) -> Result<(), (String, u8)> {
    let service = BackupService::new(pool);
    let conn_info = DatabaseConnectionInfo::parse(database_url)
        .map_err(|e| (format!("Erro ao processar DATABASE_URL: {e}"), 1))?;

    let is_restoring_to_current = match &args.into {
        Some(target) => target == &conn_info.database,
        None => false,
    };

    if is_restoring_to_current && !args.yes {
        if std::io::stdin().is_terminal() {
            println!(
                "ATENÇÃO: Você está prestes a restaurar dados sobre o banco atual ('{}').\n\
                 Isso substituirá os dados existentes.",
                conn_info.database
            );

            let confirmation: String = Input::new()
                .with_prompt(format!(
                    "Digite o nome do banco ('{}') para confirmar",
                    conn_info.database
                ))
                .interact_text()
                .map_err(|e| (format!("Erro ao ler confirmação do terminal: {e}"), 1))?;

            if confirmation.trim() != conn_info.database {
                return Err((
                    "Restauração cancelada: o nome digitado não confere com o banco atual."
                        .to_string(),
                    1,
                ));
            }
        } else {
            return Err((
                format!(
                    "Restauração sobre o banco de dados atual ('{}') requer confirmação explícita. Passe '--yes'.",
                    conn_info.database
                ),
                1,
            ));
        }
    }

    let summary = service
        .restore(RestoreInput {
            database_url: database_url.to_string(),
            file_path: args.file,
            into_database: args.into,
            yes: args.yes || is_restoring_to_current,
        })
        .await
        .map_err(|e| (e.to_string(), 2))?;

    println!("✓ Restauração concluída com sucesso!");
    println!("  Banco de dados: {}", summary.target_database);
    if summary.is_new_database {
        println!("  Status: Novo banco de dados criado.");
        if let Some(new_url) = summary.new_database_url {
            println!("  Para conectar a este novo banco, utilize:");
            println!("    DATABASE_URL={new_url}");
        }
    } else {
        println!("  Status: Restaurado sobre o banco existente.");
    }
    println!("  Duração: {:.2?}", summary.duration);

    Ok(())
}
