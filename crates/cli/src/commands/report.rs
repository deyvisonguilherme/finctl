use crate::format::OutputFormat;
use app::{CategoryReportInput, CompareCategoriesInput, MonthlyReportInput, ReportService};
use clap::{Args, Subcommand};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Row, Table};
use domain::{
    format_decimal_pt_br, CategoryComparisonReport, CategoryReportSummary, MonthlyReportItem,
    TransactionKind, UserId,
};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum ReportCommands {
    /// Relatório mensal consolidado de receitas, despesas e economia
    Monthly(MonthlyArgs),

    /// Relatório de gastos ou receitas agrupados por categoria
    Categories(CategoriesArgs),

    /// Comparativo de categorias entre meses com variação absoluta e percentual
    Compare(CompareArgs),
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

#[derive(Args, Debug)]
pub struct CategoriesArgs {
    /// Mês de referência no formato AAAA-MM (ex: 2026-10) [padrão: mês atual]
    #[arg(short, long)]
    pub month: Option<String>,

    /// Filtrar por tipo de transação (income/receita ou expense/despesa)
    #[arg(short, long)]
    pub kind: Option<TransactionKind>,

    /// Profundidade da agregação: 1 (agrupa subcategorias na pai) ou 2 (detalha por subcategoria) [padrão: 2]
    #[arg(short, long, default_value_t = 2)]
    pub depth: u32,

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

#[derive(Args, Debug)]
pub struct CompareArgs {
    /// Meses para comparar separados por vírgula (ex: 2026-08,2026-09)
    #[arg(short, long, value_delimiter = ',')]
    pub months: Option<Vec<String>>,

    /// Quantidade dos últimos N meses para comparar (ex: 2, 3, 6)
    #[arg(short, long)]
    pub last: Option<u32>,

    /// Filtrar por tipo de transação (income/receita ou expense/despesa)
    #[arg(short, long)]
    pub kind: Option<TransactionKind>,

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

#[derive(Serialize)]
struct CategoryExportItem {
    category_name: String,
    parent_name: Option<String>,
    kind: String,
    transaction_count: i64,
    amount: Decimal,
    percentage: Decimal,
}

#[derive(Serialize)]
struct CategoryExportSummary {
    kind: Option<String>,
    total_amount: Decimal,
    items: Vec<CategoryExportItem>,
}

pub async fn handle_report_command(
    pool: &PgPool,
    user_id: UserId,
    command: ReportCommands,
) -> Result<(), (String, u8)> {
    match command {
        ReportCommands::Monthly(args) => handle_monthly(pool, user_id, args).await,
        ReportCommands::Categories(args) => handle_categories(pool, user_id, args).await,
        ReportCommands::Compare(args) => handle_compare(pool, user_id, args).await,
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

async fn handle_categories(
    pool: &PgPool,
    user_id: UserId,
    args: CategoriesArgs,
) -> Result<(), (String, u8)> {
    if args.depth != 1 && args.depth != 2 {
        return Err(("O parâmetro --depth deve ser 1 ou 2.".to_string(), 1));
    }

    let service = ReportService::new(pool);
    let summary = service
        .category_report(CategoryReportInput {
            user_id,
            month: args.month.clone(),
            from_date: None,
            to_date: None,
            account_query: args.account.clone(),
            kind: args.kind,
            depth: args.depth,
            include_pending: args.include_pending,
        })
        .await
        .map_err(|e| (format!("Erro ao gerar relatório de categorias: {e}"), 1))?;

    if summary.items.is_empty() {
        if args.format == OutputFormat::Table {
            println!("Nenhuma movimentação encontrada para o período e filtros informados.");
        } else if args.format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::to_string_pretty(&CategoryExportSummary {
                    kind: args.kind.map(|k| k.as_str().to_string()),
                    total_amount: Decimal::ZERO,
                    items: vec![],
                })
                .unwrap_or_default()
            );
        } else {
            println!("category_name,parent_name,kind,transaction_count,amount,percentage");
        }
        return Ok(());
    }

    match args.format {
        OutputFormat::Table => print_categories_table(&summary, args.depth),
        OutputFormat::Json => {
            let export = CategoryExportSummary {
                kind: summary.kind.map(|k| k.as_str().to_string()),
                total_amount: summary.total_amount.as_decimal(),
                items: summary
                    .items
                    .iter()
                    .map(|i| CategoryExportItem {
                        category_name: i.category_name.clone(),
                        parent_name: i.parent_name.clone(),
                        kind: i.kind.as_str().to_string(),
                        transaction_count: i.transaction_count,
                        amount: i.total_amount.as_decimal(),
                        percentage: i.percentage,
                    })
                    .collect(),
            };
            let json = serde_json::to_string_pretty(&export)
                .map_err(|e| (format!("Erro ao serializar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            println!("category_name,parent_name,kind,transaction_count,amount,percentage");
            for i in &summary.items {
                println!(
                    "\"{}\",\"{}\",{},{},{},{}",
                    i.category_name,
                    i.parent_name.as_deref().unwrap_or(""),
                    i.kind.as_str(),
                    i.transaction_count,
                    i.total_amount.as_decimal(),
                    i.percentage
                );
            }
        }
    }

    Ok(())
}

async fn handle_compare(
    pool: &PgPool,
    user_id: UserId,
    args: CompareArgs,
) -> Result<(), (String, u8)> {
    let service = ReportService::new(pool);
    let report = service
        .compare_categories(CompareCategoriesInput {
            user_id,
            months: args.months.clone(),
            last_n: args.last,
            kind: args.kind,
            account_query: args.account.clone(),
            include_pending: args.include_pending,
        })
        .await
        .map_err(|e| (format!("Erro ao gerar comparativo: {e}"), 1))?;

    if report.rows.is_empty() {
        if args.format == OutputFormat::Table {
            println!("Nenhuma movimentação encontrada para o período e filtros informados.");
        } else if args.format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
        } else {
            println!("category_name,kind,variation_amount,variation_percent");
        }
        return Ok(());
    }

    match args.format {
        OutputFormat::Table => print_compare_table(&report),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&report)
                .map_err(|e| (format!("Erro ao serializar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            let mut header = vec!["category_name".to_string(), "kind".to_string()];
            for m in &report.months {
                header.push(m.clone());
            }
            header.push("variation_amount".to_string());
            header.push("variation_percent".to_string());
            println!("{}", header.join(","));

            for row in &report.rows {
                let mut line = vec![
                    format!("\"{}\"", row.category_name),
                    row.kind.as_str().to_string(),
                ];
                for (_, money) in &row.monthly_amounts {
                    line.push(money.as_decimal().to_string());
                }
                line.push(row.absolute_diff.to_string());
                line.push(
                    row.percent_diff
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "n/d".to_string()),
                );
                println!("{}", line.join(","));
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

fn print_categories_table(summary: &CategoryReportSummary, depth: u32) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    if depth == 1 {
        table.set_header(vec![
            Cell::new("Categoria").set_alignment(CellAlignment::Left),
            Cell::new("Tipo").set_alignment(CellAlignment::Center),
            Cell::new("Lançamentos").set_alignment(CellAlignment::Right),
            Cell::new("Total").set_alignment(CellAlignment::Right),
            Cell::new("% do Total").set_alignment(CellAlignment::Right),
        ]);
    } else {
        table.set_header(vec![
            Cell::new("Categoria").set_alignment(CellAlignment::Left),
            Cell::new("Categoria Pai").set_alignment(CellAlignment::Left),
            Cell::new("Tipo").set_alignment(CellAlignment::Center),
            Cell::new("Lançamentos").set_alignment(CellAlignment::Right),
            Cell::new("Total").set_alignment(CellAlignment::Right),
            Cell::new("% do Total").set_alignment(CellAlignment::Right),
        ]);
    }

    let mut total_tx_count = 0i64;

    for item in &summary.items {
        total_tx_count += item.transaction_count;

        let kind_color = match item.kind {
            TransactionKind::Income => Color::Green,
            TransactionKind::Expense => Color::Red,
        };

        if depth == 1 {
            table.add_row(vec![
                Cell::new(&item.category_name),
                Cell::new(item.kind.display_pt_br())
                    .set_alignment(CellAlignment::Center)
                    .fg(kind_color),
                Cell::new(item.transaction_count.to_string()).set_alignment(CellAlignment::Right),
                Cell::new(item.total_amount.format_pt_br())
                    .set_alignment(CellAlignment::Right)
                    .fg(kind_color),
                Cell::new(format!("{}%", item.percentage)).set_alignment(CellAlignment::Right),
            ]);
        } else {
            table.add_row(vec![
                Cell::new(&item.category_name),
                Cell::new(item.parent_name.as_deref().unwrap_or("-")),
                Cell::new(item.kind.display_pt_br())
                    .set_alignment(CellAlignment::Center)
                    .fg(kind_color),
                Cell::new(item.transaction_count.to_string()).set_alignment(CellAlignment::Right),
                Cell::new(item.total_amount.format_pt_br())
                    .set_alignment(CellAlignment::Right)
                    .fg(kind_color),
                Cell::new(format!("{}%", item.percentage)).set_alignment(CellAlignment::Right),
            ]);
        }
    }

    let mut total_row = Row::new();
    total_row.add_cell(Cell::new("TOTAL"));
    if depth == 2 {
        total_row.add_cell(Cell::new("-"));
    }
    total_row.add_cell(Cell::new("-").set_alignment(CellAlignment::Center));
    total_row.add_cell(Cell::new(total_tx_count.to_string()).set_alignment(CellAlignment::Right));
    total_row.add_cell(
        Cell::new(summary.total_amount.format_pt_br()).set_alignment(CellAlignment::Right),
    );
    total_row.add_cell(Cell::new("100,00%").set_alignment(CellAlignment::Right));

    table.add_row(total_row);

    println!("{table}");
}

fn print_compare_table(report: &CategoryComparisonReport) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    let mut header = vec![
        Cell::new("Categoria").set_alignment(CellAlignment::Left),
        Cell::new("Tipo").set_alignment(CellAlignment::Center),
    ];

    for m in &report.months {
        header.push(Cell::new(m).set_alignment(CellAlignment::Right));
    }
    header.push(Cell::new("Variação (R$)").set_alignment(CellAlignment::Right));
    header.push(Cell::new("Variação (%)").set_alignment(CellAlignment::Right));

    table.set_header(header);

    let mut month_totals = vec![Decimal::ZERO; report.months.len()];

    for row in &report.rows {
        let kind_color = match row.kind {
            TransactionKind::Income => Color::Green,
            TransactionKind::Expense => Color::Red,
        };

        let mut row_cells = vec![
            Cell::new(&row.category_name),
            Cell::new(row.kind.display_pt_br())
                .set_alignment(CellAlignment::Center)
                .fg(kind_color),
        ];

        for (i, (_, money)) in row.monthly_amounts.iter().enumerate() {
            month_totals[i] += money.as_decimal();
            row_cells.push(Cell::new(money.format_pt_br()).set_alignment(CellAlignment::Right));
        }

        // Color coding for variation:
        // For expenses: increase is red, decrease is green
        // For incomes: increase is green, decrease is red
        let diff_color = match row.kind {
            TransactionKind::Expense => {
                if row.absolute_diff > Decimal::ZERO {
                    Color::Red
                } else if row.absolute_diff < Decimal::ZERO {
                    Color::Green
                } else {
                    Color::White
                }
            }
            TransactionKind::Income => {
                if row.absolute_diff > Decimal::ZERO {
                    Color::Green
                } else if row.absolute_diff < Decimal::ZERO {
                    Color::Red
                } else {
                    Color::White
                }
            }
        };

        row_cells.push(
            Cell::new(format_decimal_pt_br(row.absolute_diff))
                .set_alignment(CellAlignment::Right)
                .fg(diff_color),
        );

        let pct_str = match row.percent_diff {
            Some(pct) => {
                let sign = if pct > Decimal::ZERO { "+" } else { "" };
                format!("{sign}{pct}%")
            }
            None => "n/d".to_string(),
        };

        row_cells.push(
            Cell::new(pct_str)
                .set_alignment(CellAlignment::Right)
                .fg(diff_color),
        );

        table.add_row(row_cells);
    }

    if report.rows.len() > 1 && !month_totals.is_empty() {
        let mut total_row = Row::new();
        total_row.add_cell(Cell::new("TOTAL"));
        total_row.add_cell(Cell::new("-").set_alignment(CellAlignment::Center));

        for total in &month_totals {
            total_row.add_cell(
                Cell::new(format_decimal_pt_br(*total)).set_alignment(CellAlignment::Right),
            );
        }

        let (total_diff, total_pct) = if month_totals.len() >= 2 {
            let prev = month_totals[month_totals.len() - 2];
            let curr = month_totals[month_totals.len() - 1];
            let diff = curr - prev;
            let pct = if prev > Decimal::ZERO {
                let p = (diff / prev) * Decimal::from(100);
                Some(p.round_dp(2))
            } else {
                None
            };
            (diff, pct)
        } else {
            (Decimal::ZERO, None)
        };

        total_row.add_cell(
            Cell::new(format_decimal_pt_br(total_diff)).set_alignment(CellAlignment::Right),
        );

        let total_pct_str = match total_pct {
            Some(pct) => {
                let sign = if pct > Decimal::ZERO { "+" } else { "" };
                format!("{sign}{pct}%")
            }
            None => "n/d".to_string(),
        };
        total_row.add_cell(Cell::new(total_pct_str).set_alignment(CellAlignment::Right));

        table.add_row(total_row);
    }

    println!("{table}");
}
