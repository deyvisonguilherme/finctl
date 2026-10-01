use crate::format::OutputFormat;
use app::{CategoryItem, CategoryService};
use clap::Subcommand;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use domain::{TransactionKind, UserId};
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum CategoryCommands {
    /// Adiciona uma nova categoria
    Add {
        /// Nome da categoria (ex: "Alimentação", "Mercado")
        name: String,

        /// Tipo da categoria: income (receita) ou expense (despesa)
        #[arg(short, long)]
        kind: String,

        /// Nome ou ID da categoria pai (opcional)
        #[arg(short, long)]
        parent: Option<String>,
    },

    /// Lista as categorias cadastradas
    List {
        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

pub async fn handle_category_command(
    cmd: CategoryCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = CategoryService::new(pool);

    match cmd {
        CategoryCommands::Add { name, kind, parent } => {
            let cat_kind: TransactionKind = kind.parse().map_err(|e| (format!("{e}"), 1))?;
            let category = service
                .create_category(user_id, name, cat_kind, parent.as_deref())
                .await
                .map_err(|e| match e {
                    app::AppError::Storage(storage::StorageError::UniqueViolation(msg)) => (msg, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::NotFound(n) => (n, 1),
                    other => (format!("{other}"), 2),
                })?;

            println!(
                "Categoria '{}' cadastrada com sucesso! (ID: {})",
                category.name, category.id
            );
            Ok(())
        }
        CategoryCommands::List { format } => {
            let categories = service
                .list_categories(user_id)
                .await
                .map_err(|e| (format!("{e}"), 2))?;

            match format {
                OutputFormat::Table => print_categories_table(&categories),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&categories)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record(["id", "name", "kind", "parent_id", "parent_name"])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    for cat in categories {
                        wtr.write_record([
                            cat.id.to_string(),
                            cat.name,
                            cat.kind.to_string(),
                            cat.parent_id.map(|id| id.to_string()).unwrap_or_default(),
                            cat.parent_name.unwrap_or_default(),
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

fn print_categories_table(categories: &[CategoryItem]) {
    if categories.is_empty() {
        println!("Nenhuma categoria cadastrada.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Tipo").fg(Color::Cyan),
        Cell::new("Categoria").fg(Color::Cyan),
        Cell::new("Categoria Pai").fg(Color::Cyan),
    ]);

    for cat in categories {
        let display_name = if cat.parent_id.is_some() {
            format!("  └─ {}", cat.name)
        } else {
            cat.name.clone()
        };

        let kind_cell = match cat.kind {
            TransactionKind::Income => Cell::new(cat.kind.display_pt_br()).fg(Color::Green),
            TransactionKind::Expense => Cell::new(cat.kind.display_pt_br()).fg(Color::Red),
        };

        table.add_row(vec![
            Cell::new(cat.id.to_string()),
            kind_cell,
            Cell::new(display_name),
            Cell::new(cat.parent_name.as_deref().unwrap_or("-")),
        ]);
    }

    println!("{table}");
}
