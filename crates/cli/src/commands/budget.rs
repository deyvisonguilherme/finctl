use crate::format::OutputFormat;
use app::{BudgetService, CategoryBudgetStatus};
use chrono::Local;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use domain::{format_decimal_pt_br, BudgetIndicator, Money, UserId};
use sqlx::PgPool;
use storage::BudgetDetails;

pub use crate::cli::{BudgetCommands, BudgetListArgs, BudgetSetArgs, BudgetStatusArgs};

pub async fn handle_budget_command(
    pool: &PgPool,
    user_id: UserId,
    command: BudgetCommands,
) -> Result<(), (String, u8)> {
    let service = BudgetService::new(pool);

    match command {
        BudgetCommands::Set(args) => {
            let money = Money::parse(&args.amount).map_err(|e| (format!("{e}"), 1))?;
            let budget = service
                .set_budget(user_id, args.category, money, args.month.clone())
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            let month_desc = match &budget.month {
                Some(m) => format!("para o mês {m}"),
                None => "recorrente mensal".to_string(),
            };

            println!(
                "Orçamento de {} {} definido com sucesso!",
                budget.amount.format_pt_br(),
                month_desc
            );
            Ok(())
        }
        BudgetCommands::List(args) => {
            let budgets = service
                .list_budgets(user_id, args.month.clone())
                .await
                .map_err(|e| (format!("{e}"), 2))?;

            match args.format {
                OutputFormat::Table => print_budgets_table(&budgets),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&budgets)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record(["id", "category", "amount", "month"])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for b in budgets {
                        wtr.write_record([
                            b.id.to_string(),
                            b.category_name,
                            format!("{:.2}", b.amount.as_decimal()),
                            b.month.unwrap_or_else(|| "recorrente".to_string()),
                        ])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    }
                    wtr.flush()
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                }
            }
            Ok(())
        }
        BudgetCommands::Status(args) => {
            let month = args
                .month
                .unwrap_or_else(|| Local::now().format("%Y-%m").to_string());

            let statuses = service
                .get_budget_status(user_id, month.clone())
                .await
                .map_err(|e| match e {
                    app::AppError::Validation(v) => (v, 1),
                    other => (format!("{other}"), 2),
                })?;

            match args.format {
                OutputFormat::Table => print_budget_status_table(&statuses, &month),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&statuses)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record([
                        "category",
                        "budget",
                        "consumed",
                        "remaining",
                        "percentage",
                        "indicator",
                        "is_exception",
                    ])
                    .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for s in statuses {
                        wtr.write_record([
                            s.category_name,
                            format!("{:.2}", s.budget_amount.as_decimal()),
                            format!("{:.2}", s.consumed_amount.as_decimal()),
                            format!("{:.2}", s.remaining_amount),
                            format!("{:.2}%", s.percentage),
                            s.indicator.display_pt_br().to_string(),
                            s.is_monthly_exception.to_string(),
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

fn print_budgets_table(budgets: &[BudgetDetails]) {
    if budgets.is_empty() {
        println!("Nenhum orçamento cadastrado.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("ID").fg(Color::Cyan),
            Cell::new("Categoria").fg(Color::Cyan),
            Cell::new("Limite")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("Mês").fg(Color::Cyan),
        ]);

    for b in budgets {
        let month_label = b.month.as_deref().unwrap_or("Padrão (Recorrente)");
        table.add_row(vec![
            Cell::new(b.id.to_string()),
            Cell::new(&b.category_name),
            Cell::new(b.amount.format_pt_br()).set_alignment(CellAlignment::Right),
            Cell::new(month_label),
        ]);
    }

    println!("{table}");
}

fn print_budget_status_table(statuses: &[CategoryBudgetStatus], month: &str) {
    if statuses.is_empty() {
        println!("Nenhum orçamento ativo para o mês {month}.");
        return;
    }

    println!("\nSTATUS DOS ORÇAMENTOS — Mês: {month}\n");

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Categoria").fg(Color::Cyan),
            Cell::new("Orçado")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("Consumido")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("Restante")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("% Consumo")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("Status")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Center),
        ]);

    for s in statuses {
        let (status_color, status_text) = match s.indicator {
            BudgetIndicator::Ok => (Color::Green, "OK"),
            BudgetIndicator::Warning => (Color::Yellow, "ALERTA (≥ 80%)"),
            BudgetIndicator::Exceeded => (Color::Red, "ESTOURADO"),
        };

        let remaining_color = if s.remaining_amount < rust_decimal::Decimal::ZERO {
            Color::Red
        } else {
            Color::Green
        };

        table.add_row(vec![
            Cell::new(&s.category_name),
            Cell::new(s.budget_amount.format_pt_br()).set_alignment(CellAlignment::Right),
            Cell::new(s.consumed_amount.format_pt_br()).set_alignment(CellAlignment::Right),
            Cell::new(format_decimal_pt_br(s.remaining_amount))
                .set_alignment(CellAlignment::Right)
                .fg(remaining_color),
            Cell::new(format!("{:.1}%", s.percentage))
                .set_alignment(CellAlignment::Right)
                .fg(status_color),
            Cell::new(status_text)
                .set_alignment(CellAlignment::Center)
                .fg(status_color),
        ]);
    }

    println!("{table}");
}
