use crate::cli::{
    GoalAddArgs, GoalCommands, GoalContributeArgs, GoalEditArgs, GoalListArgs, GoalRmArgs,
    GoalShowArgs,
};
use crate::format::OutputFormat;
use app::{AddContributionInput, CreateGoalInput, EditGoalInput, GoalService};
use chrono::{Local, NaiveDate};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use dialoguer::Confirm;
use domain::{Money, UserId};
use sqlx::PgPool;
use storage::AccountRepository;

pub async fn handle_goal_command(
    pool: &PgPool,
    user_id: UserId,
    command: GoalCommands,
) -> Result<(), (String, u8)> {
    let service = GoalService::new(pool);

    match command {
        GoalCommands::Add(args) => handle_goal_add(&service, user_id, args).await,
        GoalCommands::List(args) => handle_goal_list(&service, user_id, args).await,
        GoalCommands::Show(args) => handle_goal_show(&service, pool, user_id, args).await,
        GoalCommands::Contribute(args) => handle_goal_contribute(&service, user_id, args).await,
        GoalCommands::Edit(args) => handle_goal_edit(&service, user_id, args).await,
        GoalCommands::Rm(args) => handle_goal_rm(&service, user_id, args).await,
    }
}

async fn handle_goal_add(
    service: &GoalService<'_>,
    user_id: UserId,
    args: GoalAddArgs,
) -> Result<(), (String, u8)> {
    let target_amount = Money::parse(&args.target_amount).map_err(|e| (format!("{e}"), 1))?;
    let target_date = match args.target_date {
        Some(ref d) => Some(parse_date(d)?),
        None => None,
    };

    let goal = service
        .create_goal(CreateGoalInput {
            user_id,
            name: args.name,
            target_amount,
            target_date,
            account_query: args.account,
        })
        .await
        .map_err(map_app_error)?;

    println!(
        "Meta '{}' de {} criada com sucesso!",
        goal.name,
        goal.target_amount.format_pt_br()
    );
    if goal.is_account_linked() {
        println!("Modo: Vinculada a conta bancária (o saldo da conta acompanha o progresso).");
    } else {
        println!("Modo: Aportes manuais (use 'finctl goal contribute' para registrar aportes).");
    }

    Ok(())
}

async fn handle_goal_list(
    service: &GoalService<'_>,
    user_id: UserId,
    args: GoalListArgs,
) -> Result<(), (String, u8)> {
    let goals = service
        .list_goals(user_id, args.all)
        .await
        .map_err(map_app_error)?;

    match args.format {
        OutputFormat::Table => {
            if goals.is_empty() {
                if args.all {
                    println!("Nenhuma meta cadastrada.");
                } else {
                    println!(
                        "Nenhuma meta ativa encontrada (use --all para ver metas concluídas)."
                    );
                }
                return Ok(());
            }

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec![
                    "ID",
                    "Nome",
                    "Tipo",
                    "Alvo",
                    "Atual",
                    "% Concluído",
                    "Data Alvo",
                    "Aporte Nec.",
                    "Status",
                ]);

            for p in &goals {
                let status_cell = if p.is_completed {
                    Cell::new("Concluída").fg(Color::Green)
                } else {
                    Cell::new("Ativa").fg(Color::Cyan)
                };

                let kind_desc = if p.goal.is_account_linked() {
                    "Conta"
                } else {
                    "Manual"
                };

                let target_date_desc = p
                    .goal
                    .target_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "n/d".to_string());

                let monthly_needed_desc = p
                    .monthly_needed
                    .map(|m| format!("{}/mês", m.format_pt_br()))
                    .unwrap_or_else(|| "n/d".to_string());

                table.add_row(vec![
                    Cell::new(&p.goal.id.to_string()[..8]),
                    Cell::new(&p.goal.name),
                    Cell::new(kind_desc),
                    Cell::new(p.goal.target_amount.format_pt_br())
                        .set_alignment(CellAlignment::Right),
                    Cell::new(p.current_amount.format_pt_br()).set_alignment(CellAlignment::Right),
                    Cell::new(format!("{:.1}%", p.percentage)).set_alignment(CellAlignment::Right),
                    Cell::new(target_date_desc).set_alignment(CellAlignment::Center),
                    Cell::new(monthly_needed_desc).set_alignment(CellAlignment::Right),
                    status_cell,
                ]);
            }

            println!("{table}");
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&goals)
                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record([
                "id",
                "name",
                "kind",
                "target_amount",
                "current_amount",
                "percentage",
                "target_date",
                "monthly_needed",
                "status",
            ])
            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

            for p in &goals {
                let kind_desc = if p.goal.is_account_linked() {
                    "account"
                } else {
                    "manual"
                };
                let status_desc = if p.is_completed {
                    "completed"
                } else {
                    "active"
                };

                wtr.write_record([
                    p.goal.id.to_string(),
                    p.goal.name.clone(),
                    kind_desc.to_string(),
                    format!("{:.2}", p.goal.target_amount.as_decimal()),
                    format!("{:.2}", p.current_amount.as_decimal()),
                    format!("{:.2}", p.percentage),
                    p.goal
                        .target_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default(),
                    p.monthly_needed
                        .map(|m| format!("{:.2}", m.as_decimal()))
                        .unwrap_or_default(),
                    status_desc.to_string(),
                ])
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
            }

            wtr.flush()
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
        }
    }

    Ok(())
}

async fn handle_goal_show(
    service: &GoalService<'_>,
    pool: &PgPool,
    user_id: UserId,
    args: GoalShowArgs,
) -> Result<(), (String, u8)> {
    let progress = service
        .get_goal_progress(user_id, &args.goal)
        .await
        .map_err(map_app_error)?;

    let account_name = if let Some(acc_id) = progress.goal.account_id {
        let acc = AccountRepository::find_by_id_or_name(pool, user_id, &acc_id.to_string()).await;
        acc.ok().flatten().map(|a| a.name)
    } else {
        None
    };

    match args.format {
        OutputFormat::Table => {
            let mut summary_table = Table::new();
            summary_table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS);

            summary_table.set_header(vec!["Propriedade", "Detalhes"]);

            summary_table.add_row(vec![
                Cell::new("Nome da Meta"),
                Cell::new(&progress.goal.name),
            ]);
            summary_table.add_row(vec![
                Cell::new("ID da Meta"),
                Cell::new(progress.goal.id.to_string()),
            ]);

            let kind_desc = match account_name {
                Some(name) => format!("Conta vinculada ({name})"),
                None => "Aportes manuais".to_string(),
            };
            summary_table.add_row(vec![Cell::new("Modo de Controle"), Cell::new(kind_desc)]);

            let status_cell = if progress.is_completed {
                let when_str = progress
                    .goal
                    .completed_at
                    .map(|c| format!(" em {}", c.format("%Y-%m-%d %H:%M")))
                    .unwrap_or_default();
                Cell::new(format!("Concluída{when_str}")).fg(Color::Green)
            } else {
                Cell::new("Em andamento").fg(Color::Cyan)
            };
            summary_table.add_row(vec![Cell::new("Status"), status_cell]);

            summary_table.add_row(vec![
                Cell::new("Valor Alvo"),
                Cell::new(progress.goal.target_amount.format_pt_br()),
            ]);
            summary_table.add_row(vec![
                Cell::new("Valor Atual"),
                Cell::new(progress.current_amount.format_pt_br()),
            ]);
            summary_table.add_row(vec![
                Cell::new("Valor Restante"),
                Cell::new(progress.remaining_amount.format_pt_br()),
            ]);
            summary_table.add_row(vec![
                Cell::new("Percentual Atingido"),
                Cell::new(format!("{:.2}%", progress.percentage)),
            ]);

            let target_date_str = progress
                .goal
                .target_date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "n/d".to_string());
            summary_table.add_row(vec![Cell::new("Data Alvo"), Cell::new(target_date_str)]);

            let monthly_needed_str = progress
                .monthly_needed
                .map(|m| format!("{}/mês", m.format_pt_br()))
                .unwrap_or_else(|| "n/d".to_string());
            summary_table.add_row(vec![
                Cell::new("Aporte Mensal Necessário"),
                Cell::new(monthly_needed_str),
            ]);

            let rate_str = progress
                .recent_monthly_rate
                .map(|r| format!("{}/mês", r.format_pt_br()))
                .unwrap_or_else(|| "n/d".to_string());
            summary_table.add_row(vec![
                Cell::new("Média Recente (últimos 90 dias)"),
                Cell::new(rate_str),
            ]);

            let est_date_str = progress
                .estimated_completion_date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "n/d".to_string());
            summary_table.add_row(vec![
                Cell::new("Previsão de Conclusão"),
                Cell::new(est_date_str),
            ]);

            println!("{summary_table}");

            if !progress.goal.is_account_linked() {
                println!("\nHistórico de Aportes:");
                if progress.contributions.is_empty() {
                    println!("  Nenhum aporte registrado até o momento.");
                } else {
                    let mut contrib_table = Table::new();
                    contrib_table
                        .load_preset(UTF8_FULL)
                        .apply_modifier(UTF8_ROUND_CORNERS)
                        .set_header(vec!["Data", "Valor", "Observação"]);

                    for c in &progress.contributions {
                        contrib_table.add_row(vec![
                            Cell::new(c.date.format("%Y-%m-%d").to_string()),
                            Cell::new(c.amount.format_pt_br()).set_alignment(CellAlignment::Right),
                            Cell::new(c.note.as_deref().unwrap_or("-")),
                        ]);
                    }
                    println!("{contrib_table}");
                }
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&progress)
                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record([
                "id",
                "name",
                "target_amount",
                "current_amount",
                "percentage",
                "target_date",
                "monthly_needed",
                "recent_monthly_rate",
                "estimated_completion_date",
                "status",
            ])
            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

            let status_desc = if progress.is_completed {
                "completed"
            } else {
                "active"
            };

            wtr.write_record([
                progress.goal.id.to_string(),
                progress.goal.name,
                format!("{:.2}", progress.goal.target_amount.as_decimal()),
                format!("{:.2}", progress.current_amount.as_decimal()),
                format!("{:.2}", progress.percentage),
                progress
                    .goal
                    .target_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default(),
                progress
                    .monthly_needed
                    .map(|m| format!("{:.2}", m.as_decimal()))
                    .unwrap_or_default(),
                progress
                    .recent_monthly_rate
                    .map(|m| format!("{:.2}", m.as_decimal()))
                    .unwrap_or_default(),
                progress
                    .estimated_completion_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default(),
                status_desc.to_string(),
            ])
            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

            wtr.flush()
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
        }
    }

    Ok(())
}

async fn handle_goal_contribute(
    service: &GoalService<'_>,
    user_id: UserId,
    args: GoalContributeArgs,
) -> Result<(), (String, u8)> {
    let amount = Money::parse(&args.amount).map_err(|e| (format!("{e}"), 1))?;
    let date = match args.date {
        Some(ref d) => parse_date(d)?,
        None => Local::now().date_naive(),
    };

    let contrib = service
        .add_contribution(AddContributionInput {
            user_id,
            goal_identifier: args.goal.clone(),
            amount,
            date,
            note: args.note,
        })
        .await
        .map_err(map_app_error)?;

    println!(
        "Aporte de {} registrado com sucesso para a meta '{}'!",
        contrib.amount.format_pt_br(),
        args.goal
    );

    // Consulta se a meta acabou de ser concluída
    if let Ok(updated_progress) = service.get_goal_progress(user_id, &args.goal).await {
        if updated_progress.is_completed {
            println!(
                "🎉 Parabéns! O valor alvo foi atingido ({}) e a meta foi concluída!",
                updated_progress.goal.target_amount.format_pt_br()
            );
        }
    }

    Ok(())
}

async fn handle_goal_edit(
    service: &GoalService<'_>,
    user_id: UserId,
    args: GoalEditArgs,
) -> Result<(), (String, u8)> {
    let target_amount = match args.target_amount {
        Some(ref a) => Some(Money::parse(a).map_err(|e| (format!("{e}"), 1))?),
        None => None,
    };

    let target_date = if args.clear_target_date {
        Some(None)
    } else {
        match args.target_date {
            Some(ref d) => Some(Some(parse_date(d)?)),
            None => None,
        }
    };

    let goal = service
        .edit_goal(EditGoalInput {
            user_id,
            identifier: args.goal,
            name: args.name,
            target_amount,
            target_date,
            reopen: args.reopen,
        })
        .await
        .map_err(map_app_error)?;

    println!("Meta '{}' atualizada com sucesso!", goal.name);
    if args.reopen {
        println!("Meta reaberta como ativa.");
    }

    Ok(())
}

async fn handle_goal_rm(
    service: &GoalService<'_>,
    user_id: UserId,
    args: GoalRmArgs,
) -> Result<(), (String, u8)> {
    if !args.yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Tem certeza que deseja excluir a meta '{}' e seus aportes?",
                args.goal
            ))
            .default(false)
            .interact()
            .unwrap_or(false);

        if !confirmed {
            println!("Operação cancelada pelo usuário.");
            return Ok(());
        }
    }

    let deleted = service
        .delete_goal(user_id, &args.goal)
        .await
        .map_err(map_app_error)?;

    println!("Meta '{}' excluída com sucesso.", deleted.name);
    Ok(())
}

fn parse_date(input: &str) -> Result<NaiveDate, (String, u8)> {
    NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d").map_err(|e| {
        (
            format!("Data inválida '{input}'. Use o formato AAAA-MM-DD: {e}"),
            1,
        )
    })
}

fn map_app_error(e: app::AppError) -> (String, u8) {
    match e {
        app::AppError::NotFound(n) => (n, 1),
        app::AppError::Validation(v) => (v, 1),
        app::AppError::Domain(d) => (format!("{d}"), 1),
        other => (format!("{other}"), 2),
    }
}
