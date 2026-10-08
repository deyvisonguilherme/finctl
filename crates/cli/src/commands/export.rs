use app::{ListTransactionsInput, TransactionService};
use domain::UserId;
use sqlx::PgPool;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub use crate::cli::{ExportCommands, ExportFormat, ExportLocale, ExportTxArgs};

pub async fn handle_export_command(
    pool: &PgPool,
    user_id: UserId,
    command: ExportCommands,
) -> Result<(), (String, u8)> {
    match command {
        ExportCommands::Tx(args) => handle_export_tx(pool, user_id, args).await,
    }
}

async fn handle_export_tx(
    pool: &PgPool,
    user_id: UserId,
    args: ExportTxArgs,
) -> Result<(), (String, u8)> {
    let output_path = Path::new(&args.output);
    if output_path.exists() && !args.force {
        return Err((
            format!(
                "O arquivo '{}' já existe. Use a flag --force para sobrescrever.",
                args.output
            ),
            1,
        ));
    }

    let tx_service = TransactionService::new(pool);
    let transactions = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            from_date: args.from,
            to_date: args.to,
            month: args.month,
            account_query: args.account,
            category_query: args.category,
            kind: args.kind,
            status: None,
            installment_group_id: None,
            tag: None,
            limit: args.limit,
            offset: None,
            search_description: None,
            deleted: Some(false),
        })
        .await
        .map_err(|e| (format!("Erro ao buscar lançamentos: {e}"), 1))?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .map_err(|e| (format!("Erro ao criar arquivo '{}': {e}", args.output), 2))?;

    match args.format {
        ExportFormat::Json => {
            let json = serde_json::to_string_pretty(&transactions)
                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
            file.write_all(json.as_bytes())
                .map_err(|e| (format!("Erro ao gravar arquivo '{}': {e}", args.output), 2))?;
        }
        ExportFormat::Csv => {
            match args.locale {
                ExportLocale::PtBr => {
                    // Write UTF-8 BOM
                    file.write_all(&[0xEF, 0xBB, 0xBF])
                        .map_err(|e| (format!("Erro ao gravar BOM no arquivo: {e}"), 2))?;

                    // Header
                    writeln!(file, "data;descricao;valor;tipo;conta;categoria;status;id")
                        .map_err(|e| (format!("Erro ao gravar header: {e}"), 2))?;

                    for tx in &transactions {
                        let amount_str = tx.amount.as_decimal().to_string().replace('.', ",");
                        let desc_clean = tx.description.replace(';', ",");
                        writeln!(
                            file,
                            "{};{};{};{};{};{};{};{}",
                            tx.date.format("%d/%m/%Y"),
                            desc_clean,
                            amount_str,
                            tx.kind.as_str(),
                            tx.account_name,
                            tx.category_name,
                            tx.status.as_str(),
                            tx.id.as_uuid()
                        )
                        .map_err(|e| (format!("Erro ao gravar linha: {e}"), 2))?;
                    }
                }
                ExportLocale::EnUs => {
                    // Header
                    writeln!(
                        file,
                        "date,description,amount,kind,account,category,status,id"
                    )
                    .map_err(|e| (format!("Erro ao gravar header: {e}"), 2))?;

                    for tx in &transactions {
                        let desc_escaped =
                            if tx.description.contains(',') || tx.description.contains('"') {
                                format!("\"{}\"", tx.description.replace('"', "\"\""))
                            } else {
                                tx.description.clone()
                            };
                        writeln!(
                            file,
                            "{},{},{},{},\"{}\",\"{}\",{},{}",
                            tx.date.format("%Y-%m-%d"),
                            desc_escaped,
                            tx.amount.as_decimal(),
                            tx.kind.as_str(),
                            tx.account_name,
                            tx.category_name,
                            tx.status.as_str(),
                            tx.id.as_uuid()
                        )
                        .map_err(|e| (format!("Erro ao gravar linha: {e}"), 2))?;
                    }
                }
            }
        }
    }

    println!(
        "Exportação concluída com sucesso: {} lançamento(s) salvo(s) em '{}'.",
        transactions.len(),
        args.output
    );
    Ok(())
}
