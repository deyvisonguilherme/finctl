use app::{CreateTransferInput, TransferService};
use chrono::{Local, NaiveDate};
use clap::Subcommand;
use domain::{Money, UserId};
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum TransferCommands {
    /// Registra uma transferência entre contas
    Add {
        /// Conta de origem (nome ou ID)
        #[arg(long = "from")]
        from: String,

        /// Conta de destino (nome ou ID)
        #[arg(long = "to")]
        to: String,

        /// Valor da transferência (ex: 150.00 ou 150,00)
        #[arg(short = 'm', long = "amount")]
        amount: String,

        /// Data da transferência no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,

        /// Descrição personalizada opcional
        #[arg(long = "desc")]
        description: Option<String>,
    },
}

pub async fn handle_transfer_command(
    cmd: TransferCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TransferService::new(pool);

    match cmd {
        TransferCommands::Add {
            from,
            to,
            amount,
            date,
            description,
        } => {
            let money = Money::parse(&amount).map_err(|e| (format!("{e}"), 1))?;
            let tx_date = if let Some(d) = date {
                NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| {
                    (
                        format!("Data inválida '{d}'. Use o formato AAAA-MM-DD: {e}"),
                        1,
                    )
                })?
            } else {
                Local::now().date_naive()
            };

            let summary = service
                .create_transfer(CreateTransferInput {
                    user_id,
                    from_account_query: from,
                    to_account_query: to,
                    amount: money,
                    date: tx_date,
                    description,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            println!(
                "Transferência de {} de '{}' para '{}' realizada com sucesso! (Transfer ID: {})",
                summary.amount.format_pt_br(),
                summary.from_account_name,
                summary.to_account_name,
                summary.transfer_id
            );
            Ok(())
        }
    }
}
