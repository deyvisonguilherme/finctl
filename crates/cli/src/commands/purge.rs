use app::TransactionService;
use dialoguer::Confirm;
use domain::UserId;
use sqlx::PgPool;

pub use crate::cli::PurgeArgs;

pub async fn handle_purge_command(
    args: PurgeArgs,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    if !args.yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Tem certeza que deseja expurgar definitivamente os registros excluídos há mais de {}? Esta ação é irreversível.",
                args.older_than
            ))
            .default(false)
            .interact()
            .unwrap_or(false);

        if !confirmed {
            println!("Operação cancelada pelo usuário.");
            return Ok(());
        }
    }

    let service = TransactionService::new(pool);
    let summary = service
        .purge_deleted(user_id, &args.older_than)
        .await
        .map_err(|e| (e.to_string(), 1))?;

    println!("Expurgo concluído com sucesso!");
    println!("- Transações expurgadas: {}", summary.purged_transactions);
    println!("- Contas expurgadas: {}", summary.purged_accounts);
    println!("- Categorias expurgadas: {}", summary.purged_categories);

    Ok(())
}
