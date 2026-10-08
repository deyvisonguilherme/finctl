use app::{CreateTransactionInput, TransactionService};
use chrono::{Local, NaiveDate};
use domain::{Money, TransactionKind, UserId};
use sqlx::PgPool;

pub use crate::cli::IncomeCommands;

pub async fn handle_income_command(
    cmd: IncomeCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TransactionService::new(pool);

    match cmd {
        IncomeCommands::Add {
            account,
            category,
            amount,
            date,
            description,
            pending,
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

            let status = if pending {
                domain::TransactionStatus::Pending
            } else {
                domain::TransactionStatus::Paid
            };

            let tx = service
                .create_transaction(CreateTransactionInput {
                    user_id,
                    account_query: account,
                    category_query: category,
                    kind: TransactionKind::Income,
                    amount: money,
                    date: tx_date,
                    description,
                    status: Some(status),
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            let status_msg = if tx.status == domain::TransactionStatus::Pending {
                " (prevista)"
            } else {
                ""
            };

            println!(
                "Receita de {}{} registrada com sucesso! (ID: {})",
                tx.amount.format_pt_br(),
                status_msg,
                tx.id
            );
            Ok(())
        }
    }
}
