use crate::format::OutputFormat;
use app::AccountService;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use dialoguer::Confirm;
use domain::{Account, AccountKind, Money, UserId};
use sqlx::PgPool;

pub use crate::cli::AccountCommands;

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
            closing_day,
            due_day,
            credit_limit,
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

            let parsed_limit = if let Some(ref lim) = credit_limit {
                let clean = lim.replace(',', ".");
                let dec = clean
                    .parse::<rust_decimal::Decimal>()
                    .map_err(|_| (format!("Valor de limite de crédito inválido '{lim}'"), 1))?;
                Some(Money::from_decimal_non_negative(dec).map_err(|e| (format!("{e}"), 1))?)
            } else {
                None
            };

            if account_kind == AccountKind::CreditCard {
                if closing_day.is_none() {
                    return Err((
                        "Para contas do tipo 'credit_card', é obrigatório informar o dia de fechamento (--closing-day)."
                            .to_string(),
                        1,
                    ));
                }
                if due_day.is_none() {
                    return Err((
                        "Para contas do tipo 'credit_card', é obrigatório informar o dia de vencimento (--due-day)."
                            .to_string(),
                        1,
                    ));
                }
            }

            let account = service
                .create_account_with_input(app::CreateAccountInput {
                    user_id,
                    name,
                    kind: account_kind,
                    initial_balance: balance,
                    closing_day,
                    due_day,
                    credit_limit: parsed_limit,
                })
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
                    wtr.write_record([
                        "id",
                        "name",
                        "kind",
                        "initial_balance",
                        "closing_day",
                        "due_day",
                        "credit_limit",
                        "created_at",
                    ])
                    .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    for acc in accounts {
                        wtr.write_record([
                            acc.id.to_string(),
                            acc.name,
                            acc.kind.to_string(),
                            format!("{:.2}", acc.initial_balance.as_decimal()),
                            acc.closing_day.map(|d| d.to_string()).unwrap_or_default(),
                            acc.due_day.map(|d| d.to_string()).unwrap_or_default(),
                            acc.credit_limit
                                .map(|l| format!("{:.2}", l.as_decimal()))
                                .unwrap_or_default(),
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
        AccountCommands::Rm { account, yes } => {
            if !yes {
                let confirmed = Confirm::new()
                    .with_prompt(format!(
                        "Tem certeza que deseja excluir a conta '{account}'?"
                    ))
                    .default(false)
                    .interact()
                    .unwrap_or(false);

                if !confirmed {
                    println!("Operação cancelada pelo usuário.");
                    return Ok(());
                }
            }

            let deleted_acc =
                service
                    .delete_account(user_id, &account)
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        app::AppError::Validation(v) => (v, 1),
                        other => (format!("{other}"), 2),
                    })?;

            println!("Conta '{}' excluída com sucesso.", deleted_acc.name);
            Ok(())
        }
    }
}

fn print_accounts_table(accounts: &[Account]) {
    if accounts.is_empty() {
        println!("Nenhuma conta cadastrada.");
        return;
    }

    let has_credit_cards = accounts.iter().any(|a| a.kind == AccountKind::CreditCard);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    if has_credit_cards {
        table.set_header(vec![
            Cell::new("ID").fg(Color::Cyan),
            Cell::new("Nome").fg(Color::Cyan),
            Cell::new("Tipo").fg(Color::Cyan),
            Cell::new("Saldo Inicial").fg(Color::Cyan),
            Cell::new("Fechamento").fg(Color::Cyan),
            Cell::new("Vencimento").fg(Color::Cyan),
            Cell::new("Limite").fg(Color::Cyan),
        ]);

        for acc in accounts {
            let closing_str = acc
                .closing_day
                .map(|d| format!("Dia {d}"))
                .unwrap_or_else(|| "-".to_string());
            let due_str = acc
                .due_day
                .map(|d| format!("Dia {d}"))
                .unwrap_or_else(|| "-".to_string());
            let limit_str = acc
                .credit_limit
                .map(|l| l.format_pt_br())
                .unwrap_or_else(|| "-".to_string());

            table.add_row(vec![
                Cell::new(acc.id.to_string()),
                Cell::new(&acc.name),
                Cell::new(acc.kind.display_pt_br()),
                Cell::new(acc.initial_balance.format_pt_br()).fg(Color::Green),
                Cell::new(closing_str),
                Cell::new(due_str),
                Cell::new(limit_str).fg(Color::Yellow),
            ]);
        }
    } else {
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
    }

    println!("{table}");
}
