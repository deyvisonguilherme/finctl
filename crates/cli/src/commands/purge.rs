use app::TransactionService;
use clap::Args;
use dialoguer::Confirm;
use domain::UserId;
use sqlx::PgPool;

#[derive(Args, Debug)]
pub struct PurgeArgs {
    /// Tempo limite para expurgo definitivo de registros excluídos (ex: 30d, 60d, 90d, 6m, 1y)
    #[arg(long = "older-than")]
    pub older_than: String,

    /// Pular confirmação interativa
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

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
