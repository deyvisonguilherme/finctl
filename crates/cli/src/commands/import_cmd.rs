use app::{ImportCsvInput, ImportService};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use domain::UserId;
use sqlx::PgPool;

pub use crate::cli::{ImportCommands, ImportCsvArgs};

pub async fn handle_import_command(
    pool: &PgPool,
    user_id: UserId,
    command: ImportCommands,
) -> Result<(), (String, u8)> {
    match command {
        ImportCommands::Csv(args) => handle_import_csv(pool, user_id, args).await,
    }
}

async fn handle_import_csv(
    pool: &PgPool,
    user_id: UserId,
    args: ImportCsvArgs,
) -> Result<(), (String, u8)> {
    let service = ImportService::new(pool);
    let summary = service
        .import_csv(ImportCsvInput {
            user_id,
            account_query: args.account,
            file_path: args.file.clone(),
            profile_name: args.profile,
            dry_run: args.dry_run,
        })
        .await
        .map_err(|e| (format!("Erro na importação: {e}"), 1))?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Métrica").set_alignment(CellAlignment::Left),
            Cell::new("Quantidade").set_alignment(CellAlignment::Right),
        ]);

    let title = if summary.dry_run {
        "SIMULAÇÃO DE IMPORTAÇÃO (--dry-run)"
    } else {
        "RESUMO DA IMPORTAÇÃO"
    };

    println!("\n{title}\nArquivo: {}\n", args.file);

    table.add_row(vec![
        Cell::new("Total de linhas processadas"),
        Cell::new(summary.total_rows.to_string()).set_alignment(CellAlignment::Right),
    ]);

    let imported_label = if summary.dry_run {
        "Lançamentos válidos a importar"
    } else {
        "Lançamentos importados com sucesso"
    };

    table.add_row(vec![
        Cell::new(imported_label).fg(Color::Green),
        Cell::new(summary.imported_count.to_string())
            .set_alignment(CellAlignment::Right)
            .fg(Color::Green),
    ]);

    table.add_row(vec![
        Cell::new("Duplicados ignorados (idempotência)").fg(Color::Yellow),
        Cell::new(summary.duplicates_count.to_string())
            .set_alignment(CellAlignment::Right)
            .fg(Color::Yellow),
    ]);

    let errors_color = if summary.errors.is_empty() {
        Color::White
    } else {
        Color::Red
    };

    table.add_row(vec![
        Cell::new("Erros / Linhas inválidas").fg(errors_color),
        Cell::new(summary.errors.len().to_string())
            .set_alignment(CellAlignment::Right)
            .fg(errors_color),
    ]);

    println!("{table}");

    if !summary.errors.is_empty() {
        println!("\nDetalhamento dos erros:");
        for err in &summary.errors {
            println!("  - Linha {}: {}", err.line_number, err.reason);
        }
    }

    Ok(())
}
