use crate::format::OutputFormat;
use app::{BalanceReport, BalanceService};
use chrono::NaiveDate;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use domain::{format_decimal_pt_br, UserId};
use sqlx::PgPool;

pub use crate::cli::BalanceArgs;

pub async fn handle_balance_command(
    args: BalanceArgs,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = BalanceService::new(pool);

    let at_date = if let Some(d) = args.at {
        Some(NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| {
            (
                format!("Data inválida '{d}'. Use o formato AAAA-MM-DD: {e}"),
                1,
            )
        })?)
    } else {
        None
    };

    let report = service
        .get_balance(user_id, at_date, args.projected)
        .await
        .map_err(|e| (format!("{e}"), 2))?;

    match args.format {
        OutputFormat::Table => print_balance_table(&report),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&report)
                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record([
                "account_id",
                "account_name",
                "account_kind",
                "initial_balance",
                "total_income",
                "total_expense",
                "current_balance",
            ])
            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

            for acc in &report.accounts {
                wtr.write_record([
                    acc.account_id.to_string(),
                    acc.account_name.clone(),
                    acc.account_kind.to_string(),
                    format!("{:.2}", acc.initial_balance.as_decimal()),
                    format!("{:.2}", acc.total_income.as_decimal()),
                    format!("{:.2}", acc.total_expense.as_decimal()),
                    format!("{:.2}", acc.current_balance),
                ])
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
            }
            wtr.flush()
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
        }
    }

    Ok(())
}

fn print_balance_table(report: &BalanceReport) {
    if report.accounts.is_empty() {
        println!("Nenhuma conta cadastrada.");
        return;
    }

    let title = if report.projected {
        if let Some(d) = report.as_of_date {
            format!(
                "Saldo projetado (inclui previstos) até {}\n",
                d.format("%d/%m/%Y")
            )
        } else {
            "Saldo projetado consolidado (inclui previstos)\n".to_string()
        }
    } else if let Some(d) = report.as_of_date {
        format!("Saldo consolidado até {}\n", d.format("%d/%m/%Y"))
    } else {
        "".to_string()
    };

    if !title.is_empty() {
        print!("{title}");
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Conta").fg(Color::Cyan),
        Cell::new("Tipo").fg(Color::Cyan),
        Cell::new("Saldo Inicial").fg(Color::Cyan),
        Cell::new("Receitas (+)").fg(Color::Cyan),
        Cell::new("Despesas (-)").fg(Color::Cyan),
        Cell::new("Saldo Atual").fg(Color::Cyan),
    ]);

    for acc in &report.accounts {
        let balance_color = if acc.current_balance < rust_decimal::Decimal::ZERO {
            Color::Red
        } else {
            Color::Green
        };

        table.add_row(vec![
            Cell::new(&acc.account_name),
            Cell::new(acc.account_kind.display_pt_br()),
            Cell::new(acc.initial_balance.format_pt_br()),
            Cell::new(acc.total_income.format_pt_br()).fg(Color::Green),
            Cell::new(acc.total_expense.format_pt_br()).fg(Color::Red),
            Cell::new(format_decimal_pt_br(acc.current_balance)).fg(balance_color),
        ]);
    }

    // Row separator and Total Row
    let total_balance_color = if report.total_balance < rust_decimal::Decimal::ZERO {
        Color::Red
    } else {
        Color::Green
    };

    table.add_row(vec![
        Cell::new("TOTAL GERAL").fg(Color::Yellow),
        Cell::new("-").fg(Color::Yellow),
        Cell::new(format_decimal_pt_br(report.total_initial_balance)).fg(Color::Yellow),
        Cell::new(format_decimal_pt_br(report.total_income)).fg(Color::Green),
        Cell::new(format_decimal_pt_br(report.total_expense)).fg(Color::Red),
        Cell::new(format_decimal_pt_br(report.total_balance)).fg(total_balance_color),
    ]);

    println!("{table}");
}
