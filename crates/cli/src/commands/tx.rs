use crate::format::OutputFormat;
use app::{ListTransactionsInput, TransactionService};
use chrono::NaiveDate;
use clap::Subcommand;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use domain::{TransactionKind, UserId};
use sqlx::PgPool;
use storage::TransactionDetails;

#[derive(Subcommand, Debug)]
pub enum TxCommands {
    /// Lista lançamentos com filtros
    List {
        /// Data inicial no formato AAAA-MM-DD
        #[arg(long = "from")]
        from_date: Option<String>,

        /// Data final no formato AAAA-MM-DD
        #[arg(long = "to")]
        to_date: Option<String>,

        /// Mês de referência no formato AAAA-MM (ex: 2026-10)
        #[arg(short, long)]
        month: Option<String>,

        /// Filtrar por nome ou ID da conta
        #[arg(short, long)]
        account: Option<String>,

        /// Filtrar por nome ou ID da categoria
        #[arg(short, long)]
        category: Option<String>,

        /// Filtrar por tipo: income (receita) ou expense (despesa)
        #[arg(short, long)]
        kind: Option<String>,

        /// Limitar o número de registros exibidos
        #[arg(short, long)]
        limit: Option<i64>,

        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

pub async fn handle_tx_command(
    cmd: TxCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TransactionService::new(pool);

    match cmd {
        TxCommands::List {
            from_date,
            to_date,
            month,
            account,
            category,
            kind,
            limit,
            format,
        } => {
            let from_d = if let Some(d) = from_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data inicial inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let to_d = if let Some(d) = to_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data final inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let tx_kind = if let Some(k) = kind {
                Some(
                    k.parse::<TransactionKind>()
                        .map_err(|e| (format!("{e}"), 1))?,
                )
            } else {
                None
            };

            let transactions = service
                .list_transactions(ListTransactionsInput {
                    user_id,
                    from_date: from_d,
                    to_date: to_d,
                    month,
                    account_query: account,
                    category_query: category,
                    kind: tx_kind,
                    limit,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    other => (format!("{other}"), 2),
                })?;

            match format {
                OutputFormat::Table => print_transactions_table(&transactions),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&transactions)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record([
                        "id",
                        "date",
                        "kind",
                        "account",
                        "category",
                        "amount",
                        "description",
                    ])
                    .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for tx in transactions {
                        wtr.write_record([
                            tx.id.to_string(),
                            tx.date.to_string(),
                            tx.kind.to_string(),
                            tx.account_name,
                            tx.category_name,
                            format!("{:.2}", tx.amount.as_decimal()),
                            tx.description,
                        ])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    }
                    wtr.flush()
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                }
            }
            Ok(())
        }
    }
}

fn print_transactions_table(transactions: &[TransactionDetails]) {
    if transactions.is_empty() {
        println!("Nenhum lançamento encontrado para os filtros selecionados.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Data").fg(Color::Cyan),
        Cell::new("Tipo").fg(Color::Cyan),
        Cell::new("Conta").fg(Color::Cyan),
        Cell::new("Categoria").fg(Color::Cyan),
        Cell::new("Valor").fg(Color::Cyan),
        Cell::new("Descrição").fg(Color::Cyan),
    ]);

    for tx in transactions {
        let (kind_cell, amount_cell) = match tx.kind {
            TransactionKind::Income => (
                Cell::new(tx.kind.display_pt_br()).fg(Color::Green),
                Cell::new(format!("+{}", tx.amount.format_pt_br())).fg(Color::Green),
            ),
            TransactionKind::Expense => (
                Cell::new(tx.kind.display_pt_br()).fg(Color::Red),
                Cell::new(format!("-{}", tx.amount.format_pt_br())).fg(Color::Red),
            ),
        };

        table.add_row(vec![
            Cell::new(tx.id.to_string()),
            Cell::new(tx.date.to_string()),
            kind_cell,
            Cell::new(&tx.account_name),
            Cell::new(&tx.category_name),
            amount_cell,
            Cell::new(&tx.description),
        ]);
    }

    println!("{table}");
}
