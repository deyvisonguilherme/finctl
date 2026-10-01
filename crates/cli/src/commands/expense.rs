use app::{CreateInstallmentsInput, CreateTransactionInput, TransactionService};
use chrono::{Local, NaiveDate};
use clap::Subcommand;
use domain::{Money, TransactionKind, UserId};
use sqlx::PgPool;

#[derive(Subcommand, Debug)]
pub enum ExpenseCommands {
    /// Registra uma nova despesa ou compra parcelada
    Add {
        /// Nome ou ID da conta
        #[arg(short, long)]
        account: String,

        /// Nome ou ID da categoria de despesa
        #[arg(short, long)]
        category: String,

        /// Valor total da despesa (ex: 89,90 ou 89.90)
        #[arg(short = 'm', long = "amount")]
        amount: Option<String>,

        /// Valor de cada parcela (alternativa ao valor total ao usar --installments)
        #[arg(long = "installment-amount")]
        installment_amount: Option<String>,

        /// Número de parcelas (para compras parceladas, mínimo: 2)
        #[arg(short = 'i', long = "installments")]
        installments: Option<u32>,

        /// Data da despesa / primeira parcela no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,

        /// Descrição do lançamento
        #[arg(long = "desc", default_value = "")]
        description: String,

        /// Registra a despesa avulsa como prevista (pendente de realização)
        #[arg(long)]
        pending: bool,
    },
}

pub async fn handle_expense_command(
    cmd: ExpenseCommands,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    let service = TransactionService::new(pool);

    match cmd {
        ExpenseCommands::Add {
            account,
            category,
            amount,
            installment_amount,
            installments,
            date,
            description,
            pending,
        } => {
            let tx_date = if let Some(d) = date {
                NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| {
                    (
                        format!("Data inválida '{d}'. Use o formato AAAA-MM-DD: {e}"),
                        1,
                    )
                })?
            } else {
                Local::now().date_naive()
            };

            let (cat_id, date_for_budget) = if let Some(n) = installments {
                if n < 2 {
                    return Err(("O número de parcelas deve ser no mínimo 2.".to_string(), 1));
                }

                let total_m = if let Some(ref a) = amount {
                    Some(Money::parse(a).map_err(|e| (format!("{e}"), 1))?)
                } else {
                    None
                };

                let inst_m = if let Some(ref ia) = installment_amount {
                    Some(Money::parse(ia).map_err(|e| (format!("{e}"), 1))?)
                } else {
                    None
                };

                let summary = service
                    .create_installments(CreateInstallmentsInput {
                        user_id,
                        account_query: account,
                        category_query: category,
                        kind: TransactionKind::Expense,
                        total_amount: total_m,
                        installment_amount: inst_m,
                        installments_count: n,
                        start_date: tx_date,
                        description,
                    })
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        app::AppError::Validation(v) => (v, 1),
                        app::AppError::Domain(d) => (format!("{d}"), 1),
                        other => (format!("{other}"), 2),
                    })?;

                println!(
                    "Parcelamento em {}x de {} registrado com sucesso! (Total: {}, Grupo: {})",
                    summary.count,
                    summary.transactions[0].amount.format_pt_br(),
                    summary.total_amount.format_pt_br(),
                    summary.installment_group_id
                );

                (summary.transactions[0].category_id, tx_date)
            } else {
                if installment_amount.is_some() {
                    return Err((
                        "A opção '--installment-amount' só pode ser usada em conjunto com '--installments <N>'.".to_string(),
                        1,
                    ));
                }

                let amt_str = amount.ok_or_else(|| {
                    (
                        "O valor da despesa (--amount) é obrigatório.".to_string(),
                        1,
                    )
                })?;
                let money = Money::parse(&amt_str).map_err(|e| (format!("{e}"), 1))?;

                let status = if pending {
                    domain::TransactionStatus::Pending
                } else {
                    domain::TransactionStatus::Paid
                };

                let tx = service
                    .create_transaction(CreateTransactionInput {
                        user_id,
                        account_query: account,
                        category_query: category,
                        kind: TransactionKind::Expense,
                        amount: money,
                        date: tx_date,
                        description,
                        status: Some(status),
                    })
                    .await
                    .map_err(|e| match e {
                        app::AppError::NotFound(n) => (n, 1),
                        app::AppError::Validation(v) => (v, 1),
                        app::AppError::Domain(d) => (format!("{d}"), 1),
                        other => (format!("{other}"), 2),
                    })?;

                let status_msg = if tx.status == domain::TransactionStatus::Pending {
                    " (prevista)"
                } else {
                    ""
                };

                println!(
                    "Despesa de {}{} registrada com sucesso! (ID: {})",
                    tx.amount.format_pt_br(),
                    status_msg,
                    tx.id
                );

                (tx.category_id, tx.date)
            };

            // Check budget alert (never affects exit code)
            let budget_service = app::BudgetService::new(pool);
            if let Ok(Some(alert)) = budget_service
                .check_budget_after_expense(user_id, cat_id, date_for_budget)
                .await
            {
                match alert.indicator {
                    domain::BudgetIndicator::Exceeded => {
                        eprintln!(
                            "\n⚠️  ATENÇÃO: Orçamento da categoria '{}' foi ESTOURADO! (Consumido: {} de {} - {:.1}%)",
                            alert.category_name,
                            alert.consumed_amount.format_pt_br(),
                            alert.budget_amount.format_pt_br(),
                            alert.percentage
                        );
                    }
                    domain::BudgetIndicator::Warning => {
                        eprintln!(
                            "\n⚠️  AVISO: Orçamento da categoria '{}' atingiu {:.1}% do limite (Consumido: {} de {}).",
                            alert.category_name,
                            alert.percentage,
                            alert.consumed_amount.format_pt_br(),
                            alert.budget_amount.format_pt_br()
                        );
                    }
                    domain::BudgetIndicator::Ok => {}
                }
            }

            Ok(())
        }
    }
}
