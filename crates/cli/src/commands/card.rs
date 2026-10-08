use crate::format::OutputFormat;
use app::{CardInvoiceDetails, CardInvoiceSummary, CardService, PayCardInvoiceInput};
use chrono::NaiveDate;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use domain::{Money, UserId};
use sqlx::PgPool;

pub use crate::cli::{CardCommands, InvoiceCommands};

pub async fn handle_card_command(
    cmd: CardCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = CardService::new(pool);

    match cmd {
        CardCommands::Invoice { subcommand } => {
            match subcommand {
                InvoiceCommands::List { card, format } => {
                    let summaries =
                        service
                            .list_invoices(user_id, &card)
                            .await
                            .map_err(|e| match e {
                                app::AppError::NotFound(n) => (n, 1),
                                app::AppError::Validation(v) => (v, 1),
                                other => (format!("{other}"), 2),
                            })?;

                    match format {
                        OutputFormat::Table => print_invoices_table(&card, &summaries),
                        OutputFormat::Json => {
                            let json = serde_json::to_string_pretty(&summaries)
                                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                            println!("{json}");
                        }
                        OutputFormat::Csv => {
                            let mut wtr = csv::Writer::from_writer(std::io::stdout());
                            wtr.write_record([
                                "month",
                                "status",
                                "closing_date",
                                "due_date",
                                "item_count",
                                "total_amount",
                            ])
                            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                            for item in summaries {
                                wtr.write_record([
                                    item.invoice.month,
                                    item.invoice.status.to_string(),
                                    item.invoice.closing_date.to_string(),
                                    item.invoice.due_date.to_string(),
                                    item.item_count.to_string(),
                                    format!("{:.2}", item.total_amount.as_decimal()),
                                ])
                                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                            }
                            wtr.flush()
                                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                        }
                    }
                    Ok(())
                }
                InvoiceCommands::Show {
                    card,
                    month,
                    format,
                } => {
                    let details =
                        service
                            .show_invoice(user_id, &card, month)
                            .await
                            .map_err(|e| match e {
                                app::AppError::NotFound(n) => (n, 1),
                                app::AppError::Validation(v) => (v, 1),
                                other => (format!("{other}"), 2),
                            })?;

                    match format {
                        OutputFormat::Table => print_invoice_details_table(&details),
                        OutputFormat::Json => {
                            let json = serde_json::to_string_pretty(&details)
                                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
                            println!("{json}");
                        }
                        OutputFormat::Csv => {
                            let mut wtr = csv::Writer::from_writer(std::io::stdout());
                            wtr.write_record([
                                "date",
                                "description",
                                "category",
                                "amount",
                                "status",
                                "installments",
                            ])
                            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                            for tx in details.transactions {
                                let inst_str = match (tx.installment_number, tx.installment_total) {
                                    (Some(n), Some(tot)) => format!("{n}/{tot}"),
                                    _ => "-".to_string(),
                                };
                                wtr.write_record([
                                    tx.date.to_string(),
                                    tx.description,
                                    tx.category_name,
                                    format!("{:.2}", tx.amount.as_decimal()),
                                    tx.status.to_string(),
                                    inst_str,
                                ])
                                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                            }
                            wtr.flush()
                                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
                        }
                    }
                    Ok(())
                }
                InvoiceCommands::Close { card, month } => {
                    let closed_inv = service.close_invoice(user_id, &card, month).await.map_err(
                        |e| match e {
                            app::AppError::NotFound(n) => (n, 1),
                            app::AppError::Validation(v) => (v, 1),
                            other => (format!("{other}"), 2),
                        },
                    )?;

                    println!(
                        "Fatura '{}' do cartão '{}' fechada com sucesso! (Vencimento: {})",
                        closed_inv.month, card, closed_inv.due_date
                    );
                    Ok(())
                }
            }
        }
        CardCommands::Pay {
            card,
            from,
            month,
            amount,
            date,
        } => {
            let parsed_amount = if let Some(ref a) = amount {
                Some(Money::parse(a).map_err(|e| (format!("{e}"), 1))?)
            } else {
                None
            };

            let payment_date = if let Some(ref d) = date {
                Some(NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|e| {
                    (
                        format!("Data inválida '{d}'. Use o formato AAAA-MM-DD: {e}"),
                        1,
                    )
                })?)
            } else {
                None
            };

            let summary = service
                .pay_invoice(PayCardInvoiceInput {
                    user_id,
                    card_query: card,
                    from_account_query: from,
                    month,
                    amount: parsed_amount,
                    date: payment_date,
                })
                .await
                .map_err(|e| match e {
                    app::AppError::NotFound(n) => (n, 1),
                    app::AppError::Validation(v) => (v, 1),
                    app::AppError::Domain(d) => (format!("{d}"), 1),
                    other => (format!("{other}"), 2),
                })?;

            println!(
                "Pagamento de {} da fatura '{}' do cartão '{}' realizado com sucesso a partir de '{}'!",
                summary.amount_paid.format_pt_br(),
                summary.invoice_month,
                summary.card_name,
                summary.from_account_name
            );
            println!(
                "Status da Fatura: {}",
                summary.invoice_status.display_pt_br()
            );
            if summary.invoice_status != domain::InvoiceStatus::Paid {
                println!(
                    "Saldo Restante da Fatura: {}",
                    summary.remaining_balance.format_pt_br()
                );
            }
            println!("Transfer ID: {}", summary.transfer_id);
            Ok(())
        }
    }
}

fn print_invoices_table(card_name: &str, summaries: &[CardInvoiceSummary]) {
    if summaries.is_empty() {
        println!("Nenhuma fatura encontrada para o cartão '{card_name}'.");
        return;
    }

    println!("Faturas do cartão: {card_name}");
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Mês").fg(Color::Cyan),
        Cell::new("Status").fg(Color::Cyan),
        Cell::new("Fechamento").fg(Color::Cyan),
        Cell::new("Vencimento").fg(Color::Cyan),
        Cell::new("Lançamentos").fg(Color::Cyan),
        Cell::new("Total").fg(Color::Cyan),
    ]);

    for item in summaries {
        let status_color = match item.invoice.status {
            domain::InvoiceStatus::Open => Color::Green,
            domain::InvoiceStatus::Closed => Color::Yellow,
            domain::InvoiceStatus::Paid => Color::Blue,
        };

        table.add_row(vec![
            Cell::new(&item.invoice.month),
            Cell::new(item.invoice.status.display_pt_br()).fg(status_color),
            Cell::new(item.invoice.closing_date.to_string()),
            Cell::new(item.invoice.due_date.to_string()),
            Cell::new(item.item_count.to_string()),
            Cell::new(item.total_amount.format_pt_br()).fg(Color::Red),
        ]);
    }

    println!("{table}");
}

fn print_invoice_details_table(details: &CardInvoiceDetails) {
    println!(
        "Cartão: {} | Fatura: {} | Status: {} | Fechamento: {} | Vencimento: {}",
        details.account.name,
        details.invoice.month,
        details.invoice.status.display_pt_br(),
        details.invoice.closing_date,
        details.invoice.due_date
    );

    if let (Some(limit), Some(avail)) = (details.credit_limit, details.available_limit) {
        println!(
            "Limite Total: {} | Limite Disponível: {}",
            limit.format_pt_br(),
            avail.format_pt_br()
        );
    }

    println!();

    if details.transactions.is_empty() {
        println!("Nenhum lançamento nesta fatura.");
    } else {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec![
            Cell::new("Data").fg(Color::Cyan),
            Cell::new("Descrição").fg(Color::Cyan),
            Cell::new("Categoria").fg(Color::Cyan),
            Cell::new("Parcela").fg(Color::Cyan),
            Cell::new("Valor").fg(Color::Cyan),
        ]);

        for tx in &details.transactions {
            let inst_str = match (tx.installment_number, tx.installment_total) {
                (Some(n), Some(tot)) => format!("{n}/{tot}"),
                _ => "-".to_string(),
            };

            table.add_row(vec![
                Cell::new(tx.date.to_string()),
                Cell::new(&tx.description),
                Cell::new(&tx.category_name),
                Cell::new(inst_str),
                Cell::new(tx.amount.format_pt_br()).fg(Color::Red),
            ]);
        }

        println!("{table}");
    }

    println!("Total da Fatura: {}", details.total_amount.format_pt_br());
    if details.paid_amount.as_decimal() > rust_decimal::Decimal::ZERO {
        println!("Valor Pago: {}", details.paid_amount.format_pt_br());
        println!(
            "Saldo Restante: {}",
            details.remaining_amount.format_pt_br()
        );
    }
}
