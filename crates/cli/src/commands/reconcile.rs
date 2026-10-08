use crate::format::OutputFormat;
use app::{ReconcileAnalysis, ReconcileInput, ReconcileService, ReconcileStatusSummary};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use domain::UserId;
use sqlx::PgPool;

pub use crate::cli::{ReconcileArgs, ReconcileStatusArgs, ReconcileSubcommands};

pub async fn handle_reconcile_command(
    pool: &PgPool,
    user_id: UserId,
    args: ReconcileArgs,
) -> Result<(), (String, u8)> {
    let service = ReconcileService::new(pool);

    if let Some(ReconcileSubcommands::Status(status_args)) = args.command {
        return handle_reconcile_status(service, user_id, status_args).await;
    }

    let account_query = args.account.ok_or_else(|| {
        (
            "Parâmetro obrigatório: --account <CONTA>. Use `finctl reconcile --help`.".to_string(),
            1,
        )
    })?;

    let file_path = args.file.ok_or_else(|| {
        (
            "Parâmetro obrigatório: --file <ARQUIVO_CSV>. Use `finctl reconcile --help`."
                .to_string(),
            1,
        )
    })?;

    // Perform analysis first
    let analysis = service
        .reconcile(ReconcileInput {
            user_id,
            account_query: account_query.clone(),
            file_path: file_path.clone(),
            profile: args.profile.clone(),
            max_days: Some(args.days),
            apply: false,
        })
        .await
        .map_err(|e| (format!("Erro na conciliação: {e}"), 1))?;

    print_reconcile_proposal(&analysis);

    let should_apply = if args.yes {
        true
    } else if analysis.matched_pairs.is_empty() {
        false
    } else {
        println!();
        dialoguer::Confirm::new()
            .with_prompt("Deseja aplicar a conciliação para os lançamentos correspondentes?")
            .default(true)
            .interact()
            .unwrap_or(false)
    };

    if should_apply && !analysis.matched_pairs.is_empty() {
        let applied_analysis = service
            .reconcile(ReconcileInput {
                user_id,
                account_query,
                file_path,
                profile: args.profile,
                max_days: Some(args.days),
                apply: true,
            })
            .await
            .map_err(|e| (format!("Erro ao aplicar conciliação: {e}"), 1))?;

        println!(
            "\n✓ {} lançamento(s) conciliado(s) com sucesso no finctl!",
            applied_analysis.reconciled_count
        );
    } else if !analysis.matched_pairs.is_empty() {
        println!("\nOperação concluída sem gravar alterações. Use --yes para aplicar diretamente.");
    }

    Ok(())
}

fn print_reconcile_proposal(analysis: &ReconcileAnalysis) {
    let total_extrato = analysis.matched_pairs.len() + analysis.unmatched_csv_rows.len();

    println!("\nPROPOSTA DE CONCILIAÇÃO BANCÁRIA");
    println!("Conta: {}", analysis.account_name);
    println!("Total de linhas do extrato: {}", total_extrato);

    println!(
        "\n--- [MATCH] Lançamentos Correspondentes ({}) ---",
        analysis.matched_pairs.len()
    );
    if analysis.matched_pairs.is_empty() {
        println!("  Nenhum lançamento correspondente encontrado.");
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("ID Tx"),
                Cell::new("Data Tx"),
                Cell::new("Data Extrato"),
                Cell::new("Tipo"),
                Cell::new("Valor"),
                Cell::new("Descrição finctl"),
                Cell::new("Descrição Extrato"),
                Cell::new("Score"),
            ]);

        for m in &analysis.matched_pairs {
            let kind_str = match m.transaction.kind {
                domain::TransactionKind::Income => "RECEITA",
                domain::TransactionKind::Expense => "DESPESA",
            };
            let color = match m.transaction.kind {
                domain::TransactionKind::Income => Color::Green,
                domain::TransactionKind::Expense => Color::Red,
            };

            table.add_row(vec![
                Cell::new(m.transaction.id.to_string()),
                Cell::new(m.transaction.date.format("%d/%m/%Y").to_string()),
                Cell::new(m.extrato_row.date.format("%d/%m/%Y").to_string()),
                Cell::new(kind_str).fg(color),
                Cell::new(m.transaction.amount.format_pt_br()).fg(color),
                Cell::new(&m.transaction.description),
                Cell::new(&m.extrato_row.description),
                Cell::new(format!("{:.0}%", m.similarity_score * 100.0)).fg(Color::Cyan),
            ]);
        }
        println!("{table}");
    }

    println!(
        "\n--- [NOVO] Linhas do Extrato sem Correspondente ({}) ---",
        analysis.unmatched_csv_rows.len()
    );
    if analysis.unmatched_csv_rows.is_empty() {
        println!("  Nenhuma linha não casada no extrato.");
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("Data"),
                Cell::new("Tipo"),
                Cell::new("Valor"),
                Cell::new("Descrição no Extrato"),
            ]);

        for row in &analysis.unmatched_csv_rows {
            let kind_str = match row.kind {
                domain::TransactionKind::Income => "RECEITA",
                domain::TransactionKind::Expense => "DESPESA",
            };
            let color = match row.kind {
                domain::TransactionKind::Income => Color::Green,
                domain::TransactionKind::Expense => Color::Red,
            };

            table.add_row(vec![
                Cell::new(row.date.format("%d/%m/%Y").to_string()),
                Cell::new(kind_str).fg(color),
                Cell::new(row.amount.format_pt_br()).fg(color),
                Cell::new(&row.description),
            ]);
        }
        println!("{table}");
    }

    println!(
        "\n--- [PENDENTE] Lançamentos no finctl Não Encontrados no Extrato ({}) ---",
        analysis.unmatched_db_transactions.len()
    );
    if analysis.unmatched_db_transactions.is_empty() {
        println!("  Nenhum lançamento pendente no período.");
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("ID"),
                Cell::new("Data"),
                Cell::new("Tipo"),
                Cell::new("Valor"),
                Cell::new("Descrição"),
            ]);

        for tx in &analysis.unmatched_db_transactions {
            let kind_str = match tx.kind {
                domain::TransactionKind::Income => "RECEITA",
                domain::TransactionKind::Expense => "DESPESA",
            };
            let color = match tx.kind {
                domain::TransactionKind::Income => Color::Green,
                domain::TransactionKind::Expense => Color::Red,
            };

            table.add_row(vec![
                Cell::new(tx.id.to_string()),
                Cell::new(tx.date.format("%d/%m/%Y").to_string()),
                Cell::new(kind_str).fg(color),
                Cell::new(tx.amount.format_pt_br()).fg(color),
                Cell::new(&tx.description),
            ]);
        }
        println!("{table}");
    }
}

async fn handle_reconcile_status(
    service: ReconcileService<'_>,
    user_id: UserId,
    args: ReconcileStatusArgs,
) -> Result<(), (String, u8)> {
    let summary = service
        .status(user_id, args.account)
        .await
        .map_err(|e| (format!("Erro ao consultar status de conciliação: {e}"), 1))?;

    match args.format {
        OutputFormat::Table => print_status_table(&summary),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record(["id", "date", "account_id", "kind", "amount", "description"])
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

            for tx in &summary.transactions {
                wtr.write_record([
                    tx.id.to_string(),
                    tx.date.to_string(),
                    tx.account_id.to_string(),
                    tx.kind.to_string(),
                    format!("{:.2}", tx.amount.as_decimal()),
                    tx.description.clone(),
                ])
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
            }
            wtr.flush()
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
        }
    }

    Ok(())
}

fn print_status_table(summary: &ReconcileStatusSummary) {
    let title = if let Some(acc) = &summary.account_name {
        format!("STATUS DE CONCILIAÇÃO - Conta: {acc}")
    } else {
        "STATUS DE CONCILIAÇÃO - Todas as contas".to_string()
    };

    println!("\n{title}\n");
    println!(
        "Lançamentos pendentes de conciliação: {} ({})",
        summary.total_unreconciled_count,
        summary.total_unreconciled_amount.format_pt_br()
    );

    if summary.transactions.is_empty() {
        println!("Todos os lançamentos estão conciliados!");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("ID"),
            Cell::new("Data"),
            Cell::new("Tipo"),
            Cell::new("Valor").set_alignment(CellAlignment::Right),
            Cell::new("Descrição"),
        ]);

    for tx in &summary.transactions {
        let kind_str = match tx.kind {
            domain::TransactionKind::Income => "RECEITA",
            domain::TransactionKind::Expense => "DESPESA",
        };
        let color = match tx.kind {
            domain::TransactionKind::Income => Color::Green,
            domain::TransactionKind::Expense => Color::Red,
        };

        table.add_row(vec![
            Cell::new(tx.id.to_string()),
            Cell::new(tx.date.format("%d/%m/%Y").to_string()),
            Cell::new(kind_str).fg(color),
            Cell::new(tx.amount.format_pt_br())
                .set_alignment(CellAlignment::Right)
                .fg(color),
            Cell::new(&tx.description),
        ]);
    }

    println!("{table}");
}
