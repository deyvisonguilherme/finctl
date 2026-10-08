use crate::format::OutputFormat;
use app::{AuditFilter, AuditService};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use domain::AuditAction;
use sqlx::PgPool;

pub use crate::cli::{AuditCommands, AuditListArgs};

pub async fn handle_audit_command(cmd: AuditCommands, pool: &PgPool) -> Result<(), (String, u8)> {
    match cmd {
        AuditCommands::List(args) => handle_audit_list(args, pool).await,
    }
}

async fn handle_audit_list(args: AuditListArgs, pool: &PgPool) -> Result<(), (String, u8)> {
    let service = AuditService::new(pool);

    let since_dt = if let Some(since_str) = &args.since {
        if let Ok(dt) = DateTime::parse_from_rfc3339(since_str) {
            Some(dt.with_timezone(&Utc))
        } else if let Ok(date) = NaiveDate::parse_from_str(since_str, "%Y-%m-%d") {
            Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
        } else {
            return Err((
                format!(
                    "Formato de data inválido para '--since': '{}'. Use YYYY-MM-DD ou RFC3339.",
                    since_str
                ),
                1,
            ));
        }
    } else {
        None
    };

    let filter = AuditFilter {
        table_name: args.table,
        row_id: args.id,
        since: since_dt,
        limit: args.limit,
    };

    let entries = service
        .list(filter)
        .await
        .map_err(|e| (format!("Erro ao consultar logs de auditoria: {e}"), 2))?;

    match args.format {
        OutputFormat::Table => {
            if entries.is_empty() {
                println!("Nenhum registro de auditoria encontrado.");
                return Ok(());
            }

            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec![
                "ID",
                "Data/Hora",
                "Tabela",
                "Row ID",
                "Ação",
                "Ator",
                "Resumo / Alterações",
            ]);

            for entry in entries {
                let action_cell = match entry.action {
                    AuditAction::Insert => Cell::new(entry.action.as_str()).fg(Color::Green),
                    AuditAction::Update => Cell::new(entry.action.as_str()).fg(Color::Yellow),
                    AuditAction::Delete => Cell::new(entry.action.as_str()).fg(Color::Red),
                };

                let changed_str = entry.changed_at.format("%Y-%m-%d %H:%M:%S").to_string();
                let short_row_id = entry.row_id.to_string();
                let summary = entry.diff_summary();

                table.add_row(vec![
                    Cell::new(entry.id),
                    Cell::new(changed_str),
                    Cell::new(entry.table_name),
                    Cell::new(short_row_id),
                    action_cell,
                    Cell::new(entry.actor),
                    Cell::new(summary),
                ]);
            }

            println!("{table}");
            Ok(())
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&entries)
                .map_err(|e| (format!("Erro ao serializar para JSON: {e}"), 1))?;
            println!("{json}");
            Ok(())
        }
        OutputFormat::Csv => {
            println!("id,changed_at,table_name,row_id,action,actor,old,new");
            for entry in entries {
                let old_str = entry
                    .old
                    .map(|v| v.to_string())
                    .unwrap_or_default()
                    .replace('"', "\"\"");
                let new_str = entry
                    .new
                    .map(|v| v.to_string())
                    .unwrap_or_default()
                    .replace('"', "\"\"");
                let changed_str = entry.changed_at.to_rfc3339();

                println!(
                    "{},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
                    entry.id,
                    changed_str,
                    entry.table_name,
                    entry.row_id,
                    entry.action.as_str(),
                    entry.actor,
                    old_str,
                    new_str
                );
            }
            Ok(())
        }
    }
}
