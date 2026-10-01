use crate::errors::AppError;
use chrono::{Days, Local, NaiveDate, Utc};
use domain::{
    calculate_installment_dates, split_installments, Money, Transaction, TransactionId,
    TransactionKind, TransactionStatus, UserId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::{
    AccountRepository, CategoryRepository, TransactionDetails, TransactionFilter,
    TransactionRepository,
};
use uuid::Uuid;

pub struct CreateTransactionInput {
    pub user_id: UserId,
    pub account_query: String,
    pub category_query: String,
    pub kind: TransactionKind,
    pub amount: Money,
    pub date: NaiveDate,
    pub description: String,
    pub status: Option<TransactionStatus>,
}

#[derive(Default, Debug)]
pub struct ListTransactionsInput {
    pub user_id: UserId,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub month: Option<String>,
    pub account_query: Option<String>,
    pub category_query: Option<String>,
    pub kind: Option<TransactionKind>,
    pub status: Option<TransactionStatus>,
    pub installment_group_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug)]
pub struct EditTransactionInput {
    pub user_id: UserId,
    pub id: TransactionId,
    pub account_query: Option<String>,
    pub category_query: Option<String>,
    pub amount: Option<Money>,
    pub date: Option<NaiveDate>,
    pub description: Option<String>,
}

#[derive(Debug)]
pub struct CreateInstallmentsInput {
    pub user_id: UserId,
    pub account_query: String,
    pub category_query: String,
    pub kind: TransactionKind,
    pub total_amount: Option<Money>,
    pub installment_amount: Option<Money>,
    pub installments_count: u32,
    pub start_date: NaiveDate,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateInstallmentsSummary {
    pub installment_group_id: Uuid,
    pub count: u32,
    pub total_amount: Money,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug)]
pub struct EditInstallmentGroupInput {
    pub user_id: UserId,
    pub group_id: Uuid,
    pub account_query: Option<String>,
    pub category_query: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditInstallmentGroupSummary {
    pub group_id: Uuid,
    pub updated_count: usize,
    pub skipped_paid_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteInstallmentGroupSummary {
    pub group_id: Uuid,
    pub deleted_count: u64,
    pub skipped_paid_count: usize,
}

pub struct TransactionService<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_transaction(
        &self,
        input: CreateTransactionInput,
    ) -> Result<Transaction, AppError> {
        let account =
            AccountRepository::find_by_id_or_name(self.pool, input.user_id, &input.account_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Conta '{}' não encontrada.", input.account_query))
                })?;

        let category =
            CategoryRepository::find_by_id_or_name(self.pool, input.user_id, &input.category_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Categoria '{}' não encontrada.",
                        input.category_query
                    ))
                })?;

        if category.kind != input.kind {
            return Err(AppError::Validation(format!(
                "A categoria '{}' é do tipo '{}', mas o lançamento é do tipo '{}'.",
                category.name,
                category.kind.display_pt_br(),
                input.kind.display_pt_br()
            )));
        }

        let status = input.status.unwrap_or(TransactionStatus::Paid);
        let transaction = Transaction::new_full(
            input.user_id,
            account.id,
            category.id,
            input.kind,
            input.amount,
            input.date,
            input.description,
            status,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )?;

        TransactionRepository::create(self.pool, &transaction).await?;
        Ok(transaction)
    }

    pub async fn pay_transaction(
        &self,
        user_id: UserId,
        id: TransactionId,
        payment_date: Option<NaiveDate>,
    ) -> Result<Transaction, AppError> {
        let mut tx = TransactionRepository::find_by_id(self.pool, user_id, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Lançamento com ID '{id}' não encontrado."))
            })?;

        tx.mark_as_paid(payment_date)?;
        TransactionRepository::update(self.pool, &tx).await?;
        Ok(tx)
    }

    pub async fn list_transactions(
        &self,
        input: ListTransactionsInput,
    ) -> Result<Vec<TransactionDetails>, AppError> {
        let account_id = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc.id)
        } else {
            None
        };

        let category_id = if let Some(ref cat_q) = input.category_query {
            let cat = CategoryRepository::find_by_id_or_name(self.pool, input.user_id, cat_q)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Categoria '{cat_q}' não encontrada."))
                })?;
            Some(cat.id)
        } else {
            None
        };

        let (from_date, to_date) = if let Some(ref month_str) = input.month {
            let parts: Vec<&str> = month_str.split('-').collect();
            if parts.len() != 2 {
                return Err(AppError::Validation(format!(
                    "Formato de mês inválido '{month_str}'. Use o formato AAAA-MM (ex: 2026-10)."
                )));
            }
            let year: i32 = parts[0]
                .parse()
                .map_err(|_| AppError::Validation(format!("Ano inválido '{}'", parts[0])))?;
            let month: u32 = parts[1]
                .parse()
                .map_err(|_| AppError::Validation(format!("Mês inválido '{}'", parts[1])))?;
            if !(1..=12).contains(&month) {
                return Err(AppError::Validation(format!(
                    "Mês inválido '{month}'. Deve ser entre 01 e 12."
                )));
            }

            let start = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
                AppError::Validation("Data de início de mês inválida.".to_string())
            })?;

            let next_month = if month == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
            };
            let end = next_month.pred_opt().unwrap_or(start);

            (Some(start), Some(end))
        } else if input.installment_group_id.is_some() {
            (input.from_date, input.to_date)
        } else if input.from_date.is_none() && input.to_date.is_none() {
            // Padrão sem filtros de data: últimos 30 dias
            let today = Local::now().date_naive();
            let thirty_days_ago = today.checked_sub_days(Days::new(30)).unwrap_or(today);
            (Some(thirty_days_ago), Some(today))
        } else {
            (input.from_date, input.to_date)
        };

        let filter = TransactionFilter {
            user_id: input.user_id,
            from_date,
            to_date,
            account_id,
            category_id,
            kind: input.kind,
            status: input.status,
            installment_group_id: input.installment_group_id,
            limit: input.limit,
            ..Default::default()
        };

        let transactions = TransactionRepository::list_with_details(self.pool, filter).await?;
        Ok(transactions)
    }

    pub async fn create_installments(
        &self,
        input: CreateInstallmentsInput,
    ) -> Result<CreateInstallmentsSummary, AppError> {
        if input.installments_count < 2 {
            return Err(AppError::Validation(
                "O número de parcelas deve ser no mínimo 2.".to_string(),
            ));
        }

        let account =
            AccountRepository::find_by_id_or_name(self.pool, input.user_id, &input.account_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Conta '{}' não encontrada.", input.account_query))
                })?;

        let category =
            CategoryRepository::find_by_id_or_name(self.pool, input.user_id, &input.category_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Categoria '{}' não encontrada.",
                        input.category_query
                    ))
                })?;

        if category.kind != input.kind {
            return Err(AppError::Validation(format!(
                "A categoria '{}' é do tipo '{}', mas o parcelamento é do tipo '{}'.",
                category.name,
                category.kind.display_pt_br(),
                input.kind.display_pt_br()
            )));
        }

        let (total_amount, amounts) = match (input.total_amount, input.installment_amount) {
            (Some(total), None) => {
                let parts = split_installments(total, input.installments_count)?;
                (total, parts)
            }
            (None, Some(inst_amt)) => {
                let total_dec = inst_amt.as_decimal() * Decimal::from(input.installments_count);
                let total = Money::new(total_dec)?;
                let parts = vec![inst_amt; input.installments_count as usize];
                (total, parts)
            }
            (Some(_), Some(_)) => {
                return Err(AppError::Validation(
                    "Informe apenas o valor total (--amount) OU o valor da parcela (--installment-amount), não ambos.".to_string(),
                ));
            }
            (None, None) => {
                return Err(AppError::Validation(
                    "Informe o valor total (--amount) ou o valor da parcela (--installment-amount).".to_string(),
                ));
            }
        };

        let dates = calculate_installment_dates(input.start_date, input.installments_count);
        let group_id = Uuid::new_v4();
        let trimmed_desc = input.description.trim();

        let mut txs = Vec::with_capacity(input.installments_count as usize);
        for (idx, (date, amt)) in dates.into_iter().zip(amounts).enumerate() {
            let num = (idx + 1) as u32;
            let desc = if trimmed_desc.is_empty() {
                format!("({}/{})", num, input.installments_count)
            } else {
                format!("{} ({}/{})", trimmed_desc, num, input.installments_count)
            };

            let tx = Transaction::new_full(
                input.user_id,
                account.id,
                category.id,
                input.kind,
                amt,
                date,
                desc,
                TransactionStatus::Pending,
                None,
                Some(group_id),
                Some(num),
                Some(input.installments_count),
                None,
                None,
                None,
            )?;
            txs.push(tx);
        }

        TransactionRepository::create_batch(self.pool, &txs).await?;

        Ok(CreateInstallmentsSummary {
            installment_group_id: group_id,
            count: input.installments_count,
            total_amount,
            transactions: txs,
        })
    }

    pub async fn edit_installment_group(
        &self,
        input: EditInstallmentGroupInput,
    ) -> Result<EditInstallmentGroupSummary, AppError> {
        let all_txs = TransactionRepository::find_by_installment_group(
            self.pool,
            input.user_id,
            input.group_id,
        )
        .await?;

        if all_txs.is_empty() {
            return Err(AppError::NotFound(format!(
                "Grupo de parcelamento '{}' não encontrado.",
                input.group_id
            )));
        }

        let paid_count = all_txs
            .iter()
            .filter(|t| t.status == TransactionStatus::Paid)
            .count();
        let pending_txs: Vec<_> = all_txs
            .into_iter()
            .filter(|t| t.status == TransactionStatus::Pending)
            .collect();

        if pending_txs.is_empty() {
            return Err(AppError::Validation(
                "Nenhuma parcela pendente encontrada para edição (todas as parcelas do grupo já foram pagas).".to_string(),
            ));
        }

        let new_acc_id = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc.id)
        } else {
            None
        };

        let new_cat_id = if let Some(ref cat_q) = input.category_query {
            let cat = CategoryRepository::find_by_id_or_name(self.pool, input.user_id, cat_q)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Categoria '{cat_q}' não encontrada."))
                })?;

            if cat.kind != pending_txs[0].kind {
                return Err(AppError::Validation(format!(
                    "A categoria '{}' é do tipo '{}', mas as parcelas são do tipo '{}'.",
                    cat.name,
                    cat.kind.display_pt_br(),
                    pending_txs[0].kind.display_pt_br()
                )));
            }
            Some(cat.id)
        } else {
            None
        };

        let updated_count = pending_txs.len();

        for mut tx in pending_txs {
            if let Some(acc_id) = new_acc_id {
                tx.account_id = acc_id;
            }
            if let Some(cat_id) = new_cat_id {
                tx.category_id = cat_id;
            }
            if let Some(ref new_desc) = input.description {
                let trimmed = new_desc.trim();
                let num = tx.installment_number.unwrap_or(1);
                let tot = tx.installment_total.unwrap_or(1);
                tx.description = if trimmed.is_empty() {
                    format!("({num}/{tot})")
                } else {
                    format!("{trimmed} ({num}/{tot})")
                };
            }
            tx.updated_at = Utc::now();
            TransactionRepository::update(self.pool, &tx).await?;
        }

        Ok(EditInstallmentGroupSummary {
            group_id: input.group_id,
            updated_count,
            skipped_paid_count: paid_count,
        })
    }

    pub async fn delete_installment_group(
        &self,
        user_id: UserId,
        group_id: Uuid,
    ) -> Result<DeleteInstallmentGroupSummary, AppError> {
        let all_txs =
            TransactionRepository::find_by_installment_group(self.pool, user_id, group_id).await?;

        if all_txs.is_empty() {
            return Err(AppError::NotFound(format!(
                "Grupo de parcelamento '{group_id}' não encontrado."
            )));
        }

        let paid_count = all_txs
            .iter()
            .filter(|t| t.status == TransactionStatus::Paid)
            .count();
        let pending_count = all_txs
            .iter()
            .filter(|t| t.status == TransactionStatus::Pending)
            .count();

        if pending_count == 0 {
            return Err(AppError::Validation(
                "Nenhuma parcela pendente encontrada para remoção (todas as parcelas do grupo já foram pagas).".to_string(),
            ));
        }

        let deleted_count = TransactionRepository::delete_pending_by_installment_group(
            self.pool, user_id, group_id,
        )
        .await?;

        Ok(DeleteInstallmentGroupSummary {
            group_id,
            deleted_count,
            skipped_paid_count: paid_count,
        })
    }

    pub async fn edit_transaction(
        &self,
        input: EditTransactionInput,
    ) -> Result<Transaction, AppError> {
        let mut tx = TransactionRepository::find_by_id(self.pool, input.user_id, input.id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Lançamento com ID '{}' não encontrado.", input.id))
            })?;

        if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            tx.account_id = acc.id;
        }

        if let Some(ref cat_q) = input.category_query {
            let cat = CategoryRepository::find_by_id_or_name(self.pool, input.user_id, cat_q)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Categoria '{cat_q}' não encontrada."))
                })?;

            if cat.kind != tx.kind {
                return Err(AppError::Validation(format!(
                    "A categoria '{}' é do tipo '{}', mas o lançamento é do tipo '{}'.",
                    cat.name,
                    cat.kind.display_pt_br(),
                    tx.kind.display_pt_br()
                )));
            }
            tx.category_id = cat.id;
        }

        if let Some(amount) = input.amount {
            tx.amount = amount;
        }

        if let Some(date) = input.date {
            tx.date = date;
        }

        if let Some(description) = input.description {
            let trimmed = description.trim().to_string();
            if trimmed.len() > 255 {
                return Err(AppError::Validation(
                    "A descrição não pode ter mais de 255 caracteres.".to_string(),
                ));
            }
            tx.description = trimmed;
        }

        tx.updated_at = Utc::now();
        TransactionRepository::update(self.pool, &tx).await?;
        Ok(tx)
    }

    pub async fn delete_transaction(
        &self,
        user_id: UserId,
        id: TransactionId,
    ) -> Result<(), AppError> {
        let deleted = TransactionRepository::delete(self.pool, user_id, id).await?;
        if !deleted {
            return Err(AppError::NotFound(format!(
                "Lançamento com ID '{id}' não encontrado."
            )));
        }
        Ok(())
    }
}
