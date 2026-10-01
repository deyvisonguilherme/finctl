use crate::format::OutputFormat;
use app::{
    CreateRecurringInput, EditRecurringInput, RecurringService, RunRecurringInput,
    RunRecurringSummary,
};
use chrono::NaiveDate;
use clap::{Args, Subcommand};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use dialoguer::Confirm;
use domain::{Money, RecurringFrequency, RecurringRuleId, TransactionKind, UserId};
use sqlx::PgPool;
use storage::RecurringRuleDetails;

#[derive(Subcommand, Debug)]
pub enum RecurringCommands {
    /// Cadastra uma nova regra de recorrência
    Add(RecurringAddArgs),

    /// Lista as regras de recorrência cadastradas
    List(RecurringListArgs),

    /// Edita uma regra de recorrência existente
    Edit(RecurringEditArgs),

    /// Pausa uma regra de recorrência
    Pause(RecurringIdArg),

    /// Retoma uma regra de recorrência pausada
    Resume(RecurringIdArg),

    /// Remove uma regra de recorrência (não apaga lançamentos já gerados)
    Rm(RecurringRmArgs),

    /// Processa as regras de recorrência e gera os lançamentos pendentes
    Run(RecurringRunArgs),
}

#[derive(Args, Debug)]
pub struct RecurringAddArgs {
    /// Nome ou ID da conta
    #[arg(short, long)]
    pub account: String,

    /// Nome ou ID da categoria
    #[arg(short, long)]
    pub category: String,

    /// Valor monetário da recorrência (ex: 2500,00 ou 2500.00)
    #[arg(short = 'm', long = "amount")]
    pub amount: String,

    /// Tipo do lançamento: expense (despesa) ou income (receita) [padrão: expense]
    #[arg(short = 'k', long, default_value = "expense")]
    pub kind: String,

    /// Frequência da recorrência: weekly (semanal), monthly (mensal), yearly (anual) [padrão: monthly]
    #[arg(short, long, default_value = "monthly")]
    pub frequency: String,

    /// Dia do mês (1-31) ou dia da semana (1-7 para semanal)
    #[arg(short, long)]
    pub day: Option<u32>,

    /// Descrição da recorrência
    #[arg(long = "desc", default_value = "")]
    pub description: String,

    /// Data de início no formato AAAA-MM-DD (padrão: hoje)
    #[arg(long = "start")]
    pub start_date: Option<String>,

    /// Data de término no formato AAAA-MM-DD (opcional)
    #[arg(long = "end")]
    pub end_date: Option<String>,
}

#[derive(Args, Debug)]
pub struct RecurringListArgs {
    /// Exibe também as regras pausadas/inativas
    #[arg(long)]
    pub all: bool,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct RecurringEditArgs {
    /// ID da regra de recorrência
    pub id: String,

    /// Novo nome ou ID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Novo nome ou ID da categoria
    #[arg(short, long)]
    pub category: Option<String>,

    /// Novo valor monetário
    #[arg(short = 'm', long = "amount")]
    pub amount: Option<String>,

    /// Nova descrição
    #[arg(long = "desc")]
    pub description: Option<String>,

    /// Nova data de término no formato AAAA-MM-DD
    #[arg(long = "end")]
    pub end_date: Option<String>,
}

#[derive(Args, Debug)]
pub struct RecurringIdArg {
    /// ID da regra de recorrência
    pub id: String,
}

#[derive(Args, Debug)]
pub struct RecurringRmArgs {
    /// ID da regra de recorrência
    pub id: String,

    /// Pular confirmação interativa
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct RecurringRunArgs {
    /// Data limite para geração dos lançamentos no formato AAAA-MM-DD (padrão: hoje)
    #[arg(long)]
    pub until: Option<String>,

    /// Executa em modo de simulação sem salvar alterações no banco
    #[arg(long)]
    pub dry_run: bool,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

pub async fn handle_recurring_command(
    pool: &PgPool,
    user_id: UserId,
    command: RecurringCommands,
) -> Result<(), (String, u8)> {
    let service = RecurringService::new(pool);

    match command {
        RecurringCommands::Add(args) => {
            let money = Money::parse(&args.amount).map_err(|e| (format!("{e}"), 1))?;
            let kind: TransactionKind = args.kind.parse().map_err(|e| (format!("{e}"), 1))?;
            let freq: RecurringFrequency =
                args.frequency.parse().map_err(|e| (format!("{e}"), 1))?;

            let start = if let Some(d) = args.start_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data inicial inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let end = if let Some(d) = args.end_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data final inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let (dom, dow) = match freq {
                RecurringFrequency::Weekly => (None, args.day),
                RecurringFrequency::Monthly | RecurringFrequency::Yearly => (args.day, None),
            };

            let rule = service
                .create_rule(CreateRecurringInput {
                    user_id,
                    account_query: args.account,
                    category_query: args.category,
                    kind,
                    amount: money,
                    description: args.description,
                    frequency: freq,
                    day_of_month: dom,
                    day_of_week: dow,
                    start_date: start,
                    end_date: end,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            println!(
                "Recorrência '{}' cadastrada com sucesso! (ID: {})",
                rule.description, rule.id
            );
            Ok(())
        }
        RecurringCommands::List(args) => {
            let active_only = if args.all { None } else { Some(true) };
            let rules = service
                .list_rules(user_id, active_only)
                .await
                .map_err(|e| (format!("{e}"), 2))?;

            match args.format {
                OutputFormat::Table => print_recurring_table(&rules),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&rules)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record([
                        "id",
                        "description",
                        "kind",
                        "amount",
                        "frequency",
                        "day",
                        "account",
                        "category",
                        "active",
                    ])
                    .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for r in rules {
                        let day_str = r
                            .day_of_month
                            .map(|d| format!("Dia {d}"))
                            .or_else(|| r.day_of_week.map(|d| format!("Dia {d} (Semana)")))
                            .unwrap_or_default();

                        wtr.write_record([
                            r.id.to_string(),
                            r.description,
                            r.kind.to_string(),
                            format!("{:.2}", r.amount.as_decimal()),
                            r.frequency.to_string(),
                            day_str,
                            r.account_name,
                            r.category_name,
                            r.active.to_string(),
                        ])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    }
                    wtr.flush()
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                }
            }
            Ok(())
        }
        RecurringCommands::Edit(args) => {
            let id = args
                .id
                .parse::<RecurringRuleId>()
                .map_err(|e| (format!("{e}"), 1))?;
            let amount = if let Some(a) = args.amount {
                Some(Money::parse(&a).map_err(|e| (format!("{e}"), 1))?)
            } else {
                None
            };
            let end_date = if let Some(d) = args.end_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data final inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let updated = service
                .edit_rule(EditRecurringInput {
                    user_id,
                    id,
                    account_query: args.account,
                    category_query: args.category,
                    amount,
                    description: args.description,
                    end_date,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            println!("Recorrência '{}' atualizada com sucesso!", updated.id);
            Ok(())
        }
        RecurringCommands::Pause(args) => {
            let id = args
                .id
                .parse::<RecurringRuleId>()
                .map_err(|e| (format!("{e}"), 1))?;
            let rule = service.pause_rule(user_id, id).await.map_err(|e| match e {
                app::AppError::NotFound(n) => (n, 1),
                other => (format!("{other}"), 2),
            })?;

            println!("Recorrência '{}' pausada com sucesso!", rule.id);
            Ok(())
        }
        RecurringCommands::Resume(args) => {
            let id = args
                .id
                .parse::<RecurringRuleId>()
                .map_err(|e| (format!("{e}"), 1))?;
            let rule = service
                .resume_rule(user_id, id)
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    other => (format!("{other}"), 2),
                })?;

            println!("Recorrência '{}' retomada com sucesso!", rule.id);
            Ok(())
        }
        RecurringCommands::Rm(args) => {
            let id = args
                .id
                .parse::<RecurringRuleId>()
                .map_err(|e| (format!("{e}"), 1))?;

            if !args.yes {
                let confirmed = Confirm::new()
                    .with_prompt(format!(
                        "Tem certeza que deseja remover a recorrência '{id}'? (Os lançamentos já gerados não serão apagados)"
                    ))
                    .default(false)
                    .interact()
                    .unwrap_or(false);

                if !confirmed {
                    println!("Operação cancelada.");
                    return Ok(());
                }
            }

            service
                .delete_rule(user_id, id)
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    other => (format!("{other}"), 2),
                })?;

            println!("Recorrência '{id}' removida com sucesso!");
            Ok(())
        }
        RecurringCommands::Run(args) => {
            let until = if let Some(d) = args.until {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data limite inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let summary = service
                .run_recurring(RunRecurringInput {
                    user_id,
                    until_date: until,
                    dry_run: args.dry_run,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            match args.format {
                OutputFormat::Table => print_recurring_run_table(&summary),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&summary)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record([
                        "rule_id",
                        "description",
                        "account",
                        "category",
                        "kind",
                        "amount",
                        "date",
                        "status",
                    ])
                    .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for tx in &summary.transactions_generated {
                        wtr.write_record([
                            tx.rule_id.to_string(),
                            tx.rule_description.clone(),
                            tx.account_name.clone(),
                            tx.category_name.clone(),
                            tx.kind.to_string(),
                            format!("{:.2}", tx.amount.as_decimal()),
                            tx.date.to_string(),
                            tx.status.to_string(),
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

fn print_recurring_run_table(summary: &RunRecurringSummary) {
    if summary.dry_run {
        println!("[MODO DE SIMULAÇÃO] Nenhuma alteração foi persistida no banco.\n");
    }

    if summary.transactions_generated.is_empty() {
        println!(
            "Nenhum lançamento recorrente pendente para gerar até {}.",
            summary.until_date
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Data").fg(Color::Cyan),
            Cell::new("Descrição").fg(Color::Cyan),
            Cell::new("Conta").fg(Color::Cyan),
            Cell::new("Categoria").fg(Color::Cyan),
            Cell::new("Tipo").fg(Color::Cyan),
            Cell::new("Valor")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("Status")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Center),
        ]);

    for tx in &summary.transactions_generated {
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
            Cell::new(tx.date.to_string()),
            Cell::new(&tx.rule_description),
            Cell::new(&tx.account_name),
            Cell::new(&tx.category_name),
            kind_cell,
            amount_cell.set_alignment(CellAlignment::Right),
            Cell::new(tx.status.display_pt_br()).fg(Color::Yellow),
        ]);
    }

    println!("{table}");
    println!(
        "\nTotal de lançamentos gerados: {} (regras ativas avaliadas: {})",
        summary.transactions_generated.len(),
        summary.rules_evaluated
    );
}

fn print_recurring_table(rules: &[RecurringRuleDetails]) {
    if rules.is_empty() {
        println!("Nenhuma regra de recorrência encontrada.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("ID").fg(Color::Cyan),
            Cell::new("Descrição").fg(Color::Cyan),
            Cell::new("Tipo").fg(Color::Cyan),
            Cell::new("Valor")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new("Frequência").fg(Color::Cyan),
            Cell::new("Dia").fg(Color::Cyan),
            Cell::new("Conta").fg(Color::Cyan),
            Cell::new("Categoria").fg(Color::Cyan),
            Cell::new("Status")
                .fg(Color::Cyan)
                .set_alignment(CellAlignment::Center),
        ]);

    for r in rules {
        let (kind_cell, amount_cell) = match r.kind {
            TransactionKind::Income => (
                Cell::new(r.kind.display_pt_br()).fg(Color::Green),
                Cell::new(format!("+{}", r.amount.format_pt_br())).fg(Color::Green),
            ),
            TransactionKind::Expense => (
                Cell::new(r.kind.display_pt_br()).fg(Color::Red),
                Cell::new(format!("-{}", r.amount.format_pt_br())).fg(Color::Red),
            ),
        };

        let day_str = match r.frequency {
            RecurringFrequency::Monthly => format!("Dia {}", r.day_of_month.unwrap_or(1)),
            RecurringFrequency::Weekly => format!("Dia {} (Semana)", r.day_of_week.unwrap_or(1)),
            RecurringFrequency::Yearly => format!("Dia {}", r.day_of_month.unwrap_or(1)),
        };

        let status_cell = if r.active {
            Cell::new("Ativa").fg(Color::Green)
        } else {
            Cell::new("Pausada").fg(Color::DarkGrey)
        };

        table.add_row(vec![
            Cell::new(r.id.to_string()),
            Cell::new(&r.description),
            kind_cell,
            amount_cell.set_alignment(CellAlignment::Right),
            Cell::new(r.frequency.display_pt_br()),
            Cell::new(day_str),
            Cell::new(&r.account_name),
            Cell::new(&r.category_name),
            status_cell.set_alignment(CellAlignment::Center),
        ]);
    }

    println!("{table}");
}
