use crate::format::OutputFormat;
use app::AccountService;
use clap::Subcommand;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use domain::{Account, AccountKind, Money, UserId};
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum AccountCommands {
    /// Adiciona uma nova conta
    Add {
        /// Nome da conta (ex: "Nubank", "Carteira")
        name: String,

        /// Tipo da conta: checking (corrente), savings (poupança), wallet (carteira), investment (investimento)
        #[arg(short, long)]
        kind: String,

        /// Saldo inicial da conta (ex: 1500,00 ou 1500.00)
        #[arg(short, long, default_value = "0,00")]
        initial_balance: String,
    },

    /// Lista as contas cadastradas
    List {
        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

pub async fn handle_account_command(
    cmd: AccountCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = AccountService::new(pool);

    match cmd {
        AccountCommands::Add {
            name,
            kind,
            initial_balance,
        } => {
            let account_kind: AccountKind = kind.parse().map_err(|e| (format!("{e}"), 1))?;
            let balance = Money::parse(&initial_balance)
                .or_else(|_| {
                    // Try parsing as non-negative if parse failed (e.g. 0.00)
                    let clean = initial_balance.replace(',', ".");
                    let dec = clean.parse::<rust_decimal::Decimal>().map_err(|_| {
                        domain::DomainError::InvalidMoney(format!(
                            "Valor inválido '{initial_balance}'"
                        ))
                    })?;
                    Money::from_decimal_non_negative(dec)
                })
                .map_err(|e| (format!("{e}"), 1))?;

            let account = service
                .create_account(user_id, name, account_kind, balance)
                .await
                .map_err(|e| match e {
                    app::AppError::Storage(storage::StorageError::UniqueViolation(msg)) => (msg, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    app::AppError::Validation(v) => (v, 1),
                    other => (format!("{other}"), 2),
                })?;

            println!(
                "Conta '{}' cadastrada com sucesso! (ID: {})",
                account.name, account.id
            );
            Ok(())
        }
        AccountCommands::List { format } => {
            let accounts = service
                .list_accounts(user_id)
                .await
                .map_err(|e| (format!("{e}"), 2))?;

            match format {
                OutputFormat::Table => print_accounts_table(&accounts),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&accounts)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record(["id", "name", "kind", "initial_balance", "created_at"])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    for acc in accounts {
                        wtr.write_record([
                            acc.id.to_string(),
                            acc.name,
                            acc.kind.to_string(),
                            format!("{:.2}", acc.initial_balance.as_decimal()),
                            acc.created_at.to_rfc3339(),
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

fn print_accounts_table(accounts: &[Account]) {
    if accounts.is_empty() {
        println!("Nenhuma conta cadastrada.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Nome").fg(Color::Cyan),
        Cell::new("Tipo").fg(Color::Cyan),
        Cell::new("Saldo Inicial").fg(Color::Cyan),
    ]);

    for acc in accounts {
        table.add_row(vec![
            Cell::new(acc.id.to_string()),
            Cell::new(&acc.name),
            Cell::new(acc.kind.display_pt_br()),
            Cell::new(acc.initial_balance.format_pt_br()).fg(Color::Green),
        ]);
    }

    println!("{table}");
}
