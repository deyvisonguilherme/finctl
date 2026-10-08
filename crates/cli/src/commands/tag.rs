use crate::format::OutputFormat;
use app::TagService;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use domain::{TagWithUsage, UserId};
use sqlx::PgPool;

pub use crate::cli::{AddTagArgs, ListTagArgs, RmTagArgs, TagCommands};

pub async fn handle_tag_command(
    command: TagCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TagService::new(pool);

    match command {
        TagCommands::Add(args) => {
            let tag = service
                .add_tag(user_id, args.name)
                .await
                .map_err(|e| (format!("Erro ao criar tag: {e}"), 1))?;

            println!("Tag '{}' criada com sucesso! (ID: {})", tag.name, tag.id);
            Ok(())
        }
        TagCommands::List(args) => {
            let tags = service
                .list_tags(user_id)
                .await
                .map_err(|e| (format!("Erro ao listar tags: {e}"), 2))?;

            match args.format {
                OutputFormat::Table => print_tags_table(&tags),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&tags)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record(["id", "name", "usage_count", "created_at"])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for t in &tags {
                        wtr.write_record([
                            t.id.to_string(),
                            t.name.clone(),
                            t.usage_count.to_string(),
                            t.created_at.to_rfc3339(),
                        ])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    }
                    wtr.flush()
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                }
            }

            Ok(())
        }
        TagCommands::Rm(args) => {
            let force = args.yes;

            let result = match service.delete_tag(user_id, &args.name, force).await {
                Ok(res) => res,
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("Use --yes ou --force") {
                        let confirmed = dialoguer::Confirm::new()
                            .with_prompt(format!(
                                "A tag '{}' está associada a lançamentos. Deseja remover mesmo assim?",
                                args.name
                            ))
                            .default(false)
                            .interact()
                            .unwrap_or(false);

                        if confirmed {
                            service
                                .delete_tag(user_id, &args.name, true)
                                .await
                                .map_err(|e| (format!("Erro ao remover tag: {e}"), 1))?
                        } else {
                            println!("Operação cancelada pelo usuário.");
                            return Ok(());
                        }
                    } else {
                        return Err((format!("Erro ao remover tag: {e}"), 1));
                    }
                }
            };

            if result.deleted {
                println!(
                    "Tag '{}' removida com sucesso! (desvinculada de {} lançamento(s))",
                    result.tag_name, result.usage_count
                );
            } else {
                println!("Nenhuma tag foi removida.");
            }

            Ok(())
        }
    }
}

fn print_tags_table(tags: &[TagWithUsage]) {
    if tags.is_empty() {
        println!("Nenhuma tag cadastrada.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("ID"),
            Cell::new("Nome"),
            Cell::new("Lançamentos Vinculados").set_alignment(CellAlignment::Right),
            Cell::new("Criada em"),
        ]);

    for t in tags {
        table.add_row(vec![
            Cell::new(t.id.to_string()),
            Cell::new(&t.name).fg(Color::Cyan),
            Cell::new(t.usage_count.to_string()).set_alignment(CellAlignment::Right),
            Cell::new(t.created_at.format("%d/%m/%Y %H:%M").to_string()),
        ]);
    }

    println!("{table}");
}
