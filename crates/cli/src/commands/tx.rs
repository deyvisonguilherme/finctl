use crate::format::OutputFormat;
use app::{
    EditInstallmentGroupInput, EditTransactionInput, ListTransactionsInput, TransactionService,
};
use chrono::NaiveDate;
use clap::Subcommand;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use dialoguer::Confirm;
use domain::{Money, TransactionId, TransactionKind, TransactionStatus, UserId};
use sqlx::PgPool;
use storage::TransactionDetails;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum TxCommands {
    /// Lista lançamentos com filtros
    List {
        /// Data inicial no formato AAAA-MM-DD
        #[arg(long = "from")]
        from_date: Option<String>,

        /// Data final no formato AAAA-MM-DD
        #[arg(long = "to")]
        to_date: Option<String>,

        /// Mês de referência no formato AAAA-MM (ex: 2026-10)
        #[arg(short, long)]
        month: Option<String>,

        /// Filtrar por nome ou ID da conta
        #[arg(short, long)]
        account: Option<String>,

        /// Filtrar por nome ou ID da categoria
        #[arg(short, long)]
        category: Option<String>,

        /// Filtrar por tipo: income (receita) ou expense (despesa)
        #[arg(short, long)]
        kind: Option<String>,

        /// Filtrar por status: paid (realizado) ou pending (previsto)
        #[arg(long)]
        status: Option<String>,

        /// Filtrar por ID do grupo de parcelamento
        #[arg(long = "group")]
        group: Option<String>,

        /// Filtrar por tag
        #[arg(long)]
        tag: Option<String>,

        /// Limitar o número de registros exibidos
        #[arg(short, long)]
        limit: Option<i64>,

        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Associa tags a um lançamento
    Tag {
        /// ID do lançamento
        id: String,

        /// Tags a serem associadas (ex: viagem trabalho)
        #[arg(required = true, num_args = 1..)]
        tags: Vec<String>,
    },

    /// Vincula um arquivo local ou URL externa como anexo ao lançamento
    Attach {
        /// ID do lançamento
        id: String,

        /// Caminho do arquivo local ou URL externa
        path_or_uri: String,

        /// Nota descritiva do anexo (opcional)
        #[arg(short, long)]
        note: Option<String>,
    },

    /// Marca um lançamento previsto (pendente) como realizado (pago)
    Pay {
        /// ID do lançamento
        id: String,

        /// Nova data de realização no formato AAAA-MM-DD (opcional; padrão: mantém a data original)
        #[arg(short, long)]
        date: Option<String>,
    },

    /// Edita os campos de um lançamento existente ou de um grupo de parcelamento
    Edit {
        /// ID do lançamento (opcional se --group for fornecido)
        id: Option<String>,

        /// ID do grupo de parcelamento (edita apenas as parcelas ainda pendentes)
        #[arg(long = "group")]
        group: Option<String>,

        /// Novo nome ou ID da conta
        #[arg(short, long)]
        account: Option<String>,

        /// Novo nome ou ID da categoria
        #[arg(short, long)]
        category: Option<String>,

        /// Novo valor monetário (apenas para edição individual de lançamento)
        #[arg(short = 'm', long = "amount")]
        amount: Option<String>,

        /// Nova data no formato AAAA-MM-DD (apenas para edição individual de lançamento)
        #[arg(short, long)]
        date: Option<String>,

        /// Nova descrição do lançamento
        #[arg(long = "desc")]
        description: Option<String>,
    },

    /// Remove um lançamento ou parcelas pendentes de um grupo de parcelamento
    Rm {
        /// ID do lançamento (opcional se --group for fornecido)
        id: Option<String>,

        /// ID do grupo de parcelamento (remove apenas as parcelas ainda pendentes)
        #[arg(long = "group")]
        group: Option<String>,

        /// Pular a confirmação interativa
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

pub async fn handle_tx_command(
    cmd: TxCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TransactionService::new(pool);

    match cmd {
        TxCommands::List {
            from_date,
            to_date,
            month,
            account,
            category,
            kind,
            status,
            group,
            tag,
            limit,
            format,
        } => {
            let from_d = if let Some(d) = from_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data inicial inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let to_d = if let Some(d) = to_date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data final inválida '{d}': {e}"), 1))?,
                )
            } else {
                None
            };

            let tx_kind = if let Some(k) = kind {
                Some(
                    k.parse::<TransactionKind>()
                        .map_err(|e| (format!("{e}"), 1))?,
                )
            } else {
                None
            };

            let tx_status = if let Some(s) = status {
                Some(
                    s.parse::<TransactionStatus>()
                        .map_err(|e| (format!("{e}"), 1))?,
                )
            } else {
                None
            };

            let grp_id = if let Some(ref g) = group {
                Some(
                    Uuid::parse_str(g)
                        .map_err(|e| (format!("ID de grupo inválido '{g}': {e}"), 1))?,
                )
            } else {
                None
            };

            let transactions = service
                .list_transactions(ListTransactionsInput {
                    user_id,
                    from_date: from_d,
                    to_date: to_d,
                    month,
                    account_query: account,
                    category_query: category,
                    kind: tx_kind,
                    status: tx_status,
                    installment_group_id: grp_id,
                    tag,
                    limit,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    other => (format!("{other}"), 2),
                })?;

            match format {
                OutputFormat::Table => print_transactions_table(&transactions),
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&transactions)
                        .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                    println!("{json}");
                }
                OutputFormat::Csv => {
                    let mut wtr = csv::Writer::from_writer(std::io::stdout());
                    wtr.write_record([
                        "id",
                        "date",
                        "kind",
                        "account",
                        "category",
                        "amount",
                        "status",
                        "description",
                        "tags",
                    ])
                    .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

                    for tx in transactions {
                        wtr.write_record([
                            tx.id.to_string(),
                            tx.date.to_string(),
                            tx.kind.to_string(),
                            tx.account_name,
                            tx.category_name,
                            format!("{:.2}", tx.amount.as_decimal()),
                            tx.status.to_string(),
                            tx.description,
                            tx.tags.join(";"),
                        ])
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                    }
                    wtr.flush()
                        .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                }
            }
            Ok(())
        }
        TxCommands::Tag { id, tags } => {
            let tag_service = app::TagService::new(pool);
            let updated = tag_service
                .tag_transaction(user_id, &id, tags)
                .await
                .map_err(|e| (format!("Erro ao associar tags: {e}"), 1))?;

            let formatted_tags = updated
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" ");

            println!("Tags atualizadas no lançamento {id}: {formatted_tags}");
            Ok(())
        }
        TxCommands::Attach {
            id,
            path_or_uri,
            note,
        } => {
            let attachment_service = app::AttachmentService::new(pool);
            let attachment = attachment_service
                .attach(user_id, &id, &path_or_uri, note)
                .await
                .map_err(|e| (format!("Erro ao anexar arquivo/URL: {e}"), 1))?;

            println!("Anexo vinculado ao lançamento com sucesso!");
            println!("  Lançamento: {id}");
            println!("  URI:        {}", attachment.uri);
            if let Some(ref h) = attachment.sha256 {
                println!("  SHA-256:    {h}");
            }
            if let Some(ref n) = attachment.note {
                println!("  Nota:       {n}");
            }
            Ok(())
        }
        TxCommands::Pay { id, date } => {
            let tx_id = id
                .parse::<TransactionId>()
                .map_err(|e| (format!("{e}"), 1))?;
            let parsed_date = if let Some(d) = date {
                Some(
                    NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .map_err(|e| (format!("Data inválida '{d}'. Use AAAA-MM-DD: {e}"), 1))?,
                )
            } else {
                None
            };

            let paid_tx = service
                .pay_transaction(user_id, tx_id, parsed_date)
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            println!(
                "Lançamento '{}' marcado como realizado (pago) com sucesso! Data: {}",
                paid_tx.id,
                paid_tx.date.format("%d/%m/%Y")
            );
            Ok(())
        }
        TxCommands::Edit {
            id,
            group,
            account,
            category,
            amount,
            date,
            description,
        } => {
            if let Some(ref g_str) = group {
                let group_id = Uuid::parse_str(g_str)
                    .map_err(|e| (format!("ID de grupo inválido '{g_str}': {e}"), 1))?;

                let summary = service
                    .edit_installment_group(EditInstallmentGroupInput {
                        user_id,
                        group_id,
                        account_query: account,
                        category_query: category,
                        description,
                    })
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        app::AppError::Validation(v) => (v, 1),
                        app::AppError::Domain(d) => (format!("{d}"), 1),
                        other => (format!("{other}"), 2),
                    })?;

                let paid_notice = if summary.skipped_paid_count > 0 {
                    format!(
                        " ({} parcela(s) já paga(s) mantida(s) sem alteração)",
                        summary.skipped_paid_count
                    )
                } else {
                    "".to_string()
                };

                println!(
                    "{} parcela(s) pendente(s) do grupo '{}' atualizada(s) com sucesso!{}",
                    summary.updated_count, summary.group_id, paid_notice
                );
                Ok(())
            } else if let Some(ref id_str) = id {
                let tx_id = id_str
                    .parse::<TransactionId>()
                    .map_err(|e| (format!("{e}"), 1))?;
                let parsed_amount = if let Some(a) = amount {
                    Some(Money::parse(&a).map_err(|e| (format!("{e}"), 1))?)
                } else {
                    None
                };
                let parsed_date =
                    if let Some(d) = date {
                        Some(NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| {
                            (format!("Data inválida '{d}'. Use AAAA-MM-DD: {e}"), 1)
                        })?)
                    } else {
                        None
                    };

                let updated = service
                    .edit_transaction(EditTransactionInput {
                        user_id,
                        id: tx_id,
                        account_query: account,
                        category_query: category,
                        amount: parsed_amount,
                        date: parsed_date,
                        description,
                    })
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        app::AppError::Validation(v) => (v, 1),
                        app::AppError::Domain(d) => (format!("{d}"), 1),
                        other => (format!("{other}"), 2),
                    })?;

                println!("Lançamento '{}' atualizado com sucesso!", updated.id);
                Ok(())
            } else {
                Err((
                    "Informe o ID do lançamento ou a flag '--group <ID>' para editar um parcelamento."
                        .to_string(),
                    1,
                ))
            }
        }
        TxCommands::Rm { id, group, yes } => {
            if let Some(ref g_str) = group {
                let group_id = Uuid::parse_str(g_str)
                    .map_err(|e| (format!("ID de grupo inválido '{g_str}': {e}"), 1))?;

                if !yes {
                    let confirmed = Confirm::new()
                        .with_prompt(format!(
                            "Tem certeza que deseja remover as parcelas pendentes do grupo '{group_id}'? (Parcelas já pagas não serão apagadas)"
                        ))
                        .default(false)
                        .interact()
                        .unwrap_or(false);

                    if !confirmed {
                        println!("Operação cancelada.");
                        return Ok(());
                    }
                }

                let summary = service
                    .delete_installment_group(user_id, group_id)
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        app::AppError::Validation(v) => (v, 1),
                        other => (format!("{other}"), 2),
                    })?;

                let paid_notice = if summary.skipped_paid_count > 0 {
                    format!(
                        " ({} parcela(s) já paga(s) mantida(s) intocada(s))",
                        summary.skipped_paid_count
                    )
                } else {
                    "".to_string()
                };

                println!(
                    "{} parcela(s) pendente(s) do grupo '{}' removida(s) com sucesso!{}",
                    summary.deleted_count, summary.group_id, paid_notice
                );
                Ok(())
            } else if let Some(ref id_str) = id {
                let tx_id = id_str
                    .parse::<TransactionId>()
                    .map_err(|e| (format!("{e}"), 1))?;

                if !yes {
                    let confirmed = Confirm::new()
                        .with_prompt(format!(
                            "Tem certeza que deseja remover o lançamento '{tx_id}'?"
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
                    .delete_transaction(user_id, tx_id)
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        other => (format!("{other}"), 2),
                    })?;

                println!("Lançamento '{tx_id}' removido com sucesso!");
                Ok(())
            } else {
                Err((
                    "Informe o ID do lançamento ou a flag '--group <ID>' para remover um parcelamento."
                        .to_string(),
                    1,
                ))
            }
        }
    }
}

fn print_transactions_table(transactions: &[TransactionDetails]) {
    if transactions.is_empty() {
        println!("Nenhum lançamento encontrado para os filtros selecionados.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Data").fg(Color::Cyan),
        Cell::new("Tipo").fg(Color::Cyan),
        Cell::new("Conta").fg(Color::Cyan),
        Cell::new("Categoria").fg(Color::Cyan),
        Cell::new("Valor").fg(Color::Cyan),
        Cell::new("Status").fg(Color::Cyan),
        Cell::new("Descrição").fg(Color::Cyan),
        Cell::new("Tags").fg(Color::Cyan),
    ]);

    for tx in transactions {
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

        let status_cell = match tx.status {
            TransactionStatus::Paid => Cell::new("pago").fg(Color::Green),
            TransactionStatus::Pending => Cell::new("previsto").fg(Color::Yellow),
        };

        let tags_str = if tx.tags.is_empty() {
            "-".to_string()
        } else {
            tx.tags
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" ")
        };

        table.add_row(vec![
            Cell::new(tx.id.to_string()),
            Cell::new(tx.date.to_string()),
            kind_cell,
            Cell::new(&tx.account_name),
            Cell::new(&tx.category_name),
            amount_cell,
            status_cell,
            Cell::new(&tx.description),
            Cell::new(tags_str).fg(Color::Magenta),
        ]);
    }

    println!("{table}");
}
