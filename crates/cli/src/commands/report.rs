use crate::format::OutputFormat;
use app::{MonthlyReportInput, ReportService};
use clap::{Args, Subcommand};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Row, Table};
use domain::{format_decimal_pt_br, MonthlyReportItem, UserId};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum ReportCommands {
    /// Relatório mensal consolidado de receitas, despesas e economia
    Monthly(MonthlyArgs),
}

#[derive(Args, Debug)]
pub struct MonthlyArgs {
    /// Mês de referência no formato AAAA-MM (ex: 2026-10) [padrão: mês atual]
    #[arg(short, long)]
    pub month: Option<String>,

    /// Ano completo para visualização mês a mês (ex: 2026)
    #[arg(short, long)]
    pub year: Option<i32>,

    /// Filtrar por nome ou UUID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Incluir lançamentos previstos/pendentes no relatório
    #[arg(long)]
    pub include_pending: bool,

    /// Formato de saída dos dados
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Serialize)]
struct MonthlyExportItem {
    month: String,
    income: Decimal,
    expense: Decimal,
    net_balance: Decimal,
    savings_rate: Decimal,
}

pub async fn handle_report_command(
    pool: &PgPool,
    user_id: UserId,
    command: ReportCommands,
) -> Result<(), (String, u8)> {
    match command {
        ReportCommands::Monthly(args) => handle_monthly(pool, user_id, args).await,
    }
}

async fn handle_monthly(
    pool: &PgPool,
    user_id: UserId,
    args: MonthlyArgs,
) -> Result<(), (String, u8)> {
    let service = ReportService::new(pool);
    let items = service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: args.month.clone(),
            year: args.year,
            account_query: args.account.clone(),
            include_pending: args.include_pending,
        })
        .await
        .map_err(|e| (format!("Erro ao gerar relatório mensal: {e}"), 1))?;

    if items.is_empty() {
        if args.format == OutputFormat::Table {
            println!("Nenhuma movimentação encontrada para o período informado.");
        } else if args.format == OutputFormat::Json {
            println!("[]");
        } else {
            println!("month,income,expense,net_balance,savings_rate");
        }
        return Ok(());
    }

    match args.format {
        OutputFormat::Table => print_monthly_table(&items, args.year.is_some() || items.len() > 1),
        OutputFormat::Json => {
            let export_items: Vec<MonthlyExportItem> = items
                .iter()
                .map(|i| MonthlyExportItem {
                    month: i.month.clone(),
                    income: i.total_income.as_decimal(),
                    expense: i.total_expense.as_decimal(),
                    net_balance: i.net_balance,
                    savings_rate: i.savings_rate,
                })
                .collect();
            let json = serde_json::to_string_pretty(&export_items)
                .map_err(|e| (format!("Erro ao serializar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            println!("month,income,expense,net_balance,savings_rate");
            for i in &items {
                println!(
                    "{},{},{},{},{}",
                    i.month,
                    i.total_income.as_decimal(),
                    i.total_expense.as_decimal(),
                    i.net_balance,
                    i.savings_rate
                );
            }
        }
    }

    Ok(())
}

fn print_monthly_table(items: &[MonthlyReportItem], show_total: bool) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Mês").set_alignment(CellAlignment::Left),
            Cell::new("Receitas").set_alignment(CellAlignment::Right),
            Cell::new("Despesas").set_alignment(CellAlignment::Right),
            Cell::new("Saldo Líquido").set_alignment(CellAlignment::Right),
            Cell::new("Poupança (%)").set_alignment(CellAlignment::Right),
        ]);

    let mut sum_income = Decimal::ZERO;
    let mut sum_expense = Decimal::ZERO;

    for item in items {
        sum_income += item.total_income.as_decimal();
        sum_expense += item.total_expense.as_decimal();

        let net_color = if item.net_balance >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };

        table.add_row(vec![
            Cell::new(&item.month),
            Cell::new(item.total_income.format_pt_br())
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
            Cell::new(item.total_expense.format_pt_br())
                .set_alignment(CellAlignment::Right)
                .fg(Color::Red),
            Cell::new(format_decimal_pt_br(item.net_balance))
                .set_alignment(CellAlignment::Right)
                .fg(net_color),
            Cell::new(format!("{}%", item.savings_rate)).set_alignment(CellAlignment::Right),
        ]);
    }

    if show_total && items.len() > 1 {
        let total_net = sum_income - sum_expense;
        let total_savings_rate = if sum_income > Decimal::ZERO {
            let rate = (total_net / sum_income) * Decimal::from(100);
            rate.round_dp(2)
        } else {
            Decimal::ZERO
        };

        let net_color = if total_net >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };

        let mut total_row = Row::new();
        total_row.add_cell(Cell::new("TOTAL"));
        total_row.add_cell(
            Cell::new(format_decimal_pt_br(sum_income))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green),
        );
        total_row.add_cell(
            Cell::new(format_decimal_pt_br(sum_expense))
                .set_alignment(CellAlignment::Right)
                .fg(Color::Red),
        );
        total_row.add_cell(
            Cell::new(format_decimal_pt_br(total_net))
                .set_alignment(CellAlignment::Right)
                .fg(net_color),
        );
        total_row.add_cell(
            Cell::new(format!("{total_savings_rate}%")).set_alignment(CellAlignment::Right),
        );

        table.add_row(total_row);
    }

    println!("{table}");
}
