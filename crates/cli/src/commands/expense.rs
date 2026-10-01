use app::{CreateTransactionInput, TransactionService};
use chrono::{Local, NaiveDate};
use clap::Subcommand;
use domain::{Money, TransactionKind, UserId};
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum ExpenseCommands {
    /// Registra uma nova despesa
    Add {
        /// Nome ou ID da conta
        #[arg(short, long)]
        account: String,

        /// Nome ou ID da categoria de despesa
        #[arg(short, long)]
        category: String,

        /// Valor da despesa (ex: 89,90 ou 89.90)
        #[arg(short = 'm', long = "amount")]
        amount: String,

        /// Data da despesa no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,

        /// Descrição do lançamento
        #[arg(long = "desc", default_value = "")]
        description: String,
    },
}

pub async fn handle_expense_command(
    cmd: ExpenseCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TransactionService::new(pool);

    match cmd {
        ExpenseCommands::Add {
            account,
            category,
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

            let tx = service
                .create_transaction(CreateTransactionInput {
                    user_id,
                    account_query: account,
                    category_query: category,
                    kind: TransactionKind::Expense,
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
                "Despesa de {} registrada com sucesso! (ID: {})",
                tx.amount.format_pt_br(),
                tx.id
            );
            Ok(())
        }
    }
}
