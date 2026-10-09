use crate::errors::AppError;
use chrono::{Days, Local, NaiveDate, Utc};
use domain::{
    calculate_installment_dates, split_installments, AccountKind, Money, Transaction,
    TransactionId, TransactionKind, TransactionStatus, UserId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::{
    AccountRepository, CardInvoiceRepository, CategoryRepository, PurgeSummary, TransactionDetails,
    TransactionFilter, TransactionRepository,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub tag: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub search_description: Option<String>,
    pub deleted: Option<bool>,
    pub all_time: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaginatedTransactions {
    pub items: Vec<TransactionDetails>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

        if account.kind == AccountKind::CreditCard {
            CardInvoiceRepository::get_or_create_for_transaction(
                self.pool,
                input.user_id,
                &account,
                input.date,
            )
            .await?;
        }

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

    async fn build_filter(
        &self,
        input: &ListTransactionsInput,
    ) -> Result<TransactionFilter, AppError> {
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
        } else if input.installment_group_id.is_some()
            || input.deleted == Some(true)
            || input.all_time
        {
            (input.from_date, input.to_date)
        } else if input.from_date.is_none() && input.to_date.is_none() {
            // Padrão sem filtros de data: últimos 30 dias se busca não estiver ativa
            if input.search_description.is_some() {
                (None, None)
            } else {
                let today = Local::now().date_naive();
                let thirty_days_ago = today.checked_sub_days(Days::new(30)).unwrap_or(today);
                (Some(thirty_days_ago), Some(today))
            }
        } else {
            (input.from_date, input.to_date)
        };

        Ok(TransactionFilter {
            user_id: input.user_id,
            from_date,
            to_date,
            account_id,
            category_id,
            kind: input.kind,
            status: input.status,
            installment_group_id: input.installment_group_id,
            tag: input.tag.clone(),
            limit: input.limit,
            offset: input.offset,
            description_query: input.search_description.clone(),
            deleted: input.deleted,
            ..Default::default()
        })
    }

    pub async fn list_transactions(
        &self,
        input: ListTransactionsInput,
    ) -> Result<Vec<TransactionDetails>, AppError> {
        let filter = self.build_filter(&input).await?;
        let transactions = TransactionRepository::list_with_details(self.pool, filter).await?;
        Ok(transactions)
    }

    pub async fn count_transactions(&self, input: ListTransactionsInput) -> Result<i64, AppError> {
        let filter = self.build_filter(&input).await?;
        let count = TransactionRepository::count_with_details(self.pool, filter).await?;
        Ok(count)
    }

    pub async fn list_transactions_paginated(
        &self,
        input: ListTransactionsInput,
    ) -> Result<PaginatedTransactions, AppError> {
        let page_size = input.limit.unwrap_or(15).max(1);
        let offset = input.offset.unwrap_or(0).max(0);
        let current_page = (offset / page_size) + 1;

        let mut count_input = input.clone();
        count_input.limit = None;
        count_input.offset = None;
        let count_filter = self.build_filter(&count_input).await?;
        let total_count =
            TransactionRepository::count_with_details(self.pool, count_filter).await?;

        let mut list_filter = self.build_filter(&input).await?;
        list_filter.limit = Some(page_size);
        list_filter.offset = Some(offset);
        let items = TransactionRepository::list_with_details(self.pool, list_filter).await?;

        let total_pages = if total_count == 0 {
            1
        } else {
            (total_count + page_size - 1) / page_size
        };

        Ok(PaginatedTransactions {
            items,
            total_count,
            page: current_page,
            page_size,
            total_pages,
        })
    }

    pub async fn pay_multiple_transactions(
        &self,
        user_id: UserId,
        tx_ids: &[TransactionId],
        payment_date: Option<NaiveDate>,
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for &id in tx_ids {
            self.pay_transaction(user_id, id, payment_date).await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn delete_multiple_transactions(
        &self,
        user_id: UserId,
        tx_ids: &[TransactionId],
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for &id in tx_ids {
            self.delete_transaction(user_id, id).await?;
            count += 1;
        }
        Ok(count)
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

        if account.kind == AccountKind::CreditCard {
            for tx in &txs {
                CardInvoiceRepository::get_or_create_for_transaction(
                    self.pool,
                    input.user_id,
                    &account,
                    tx.date,
                )
                .await?;
            }
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

        let deleted_count = TransactionRepository::soft_delete_pending_by_installment_group(
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

        if tx.transfer_id.is_some() {
            return Err(AppError::Validation(
                "Lançamentos de transferência não podem ser editados individualmente. Para alterar, remova a transferência e crie uma nova com os dados corretos.".to_string(),
            ));
        }

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
        let tx = TransactionRepository::find_by_id(self.pool, user_id, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Lançamento com ID '{id}' não encontrado."))
            })?;

        if let Some(transfer_id) = tx.transfer_id {
            TransactionRepository::soft_delete_by_transfer_id(self.pool, user_id, transfer_id)
                .await?;
        } else {
            let deleted = TransactionRepository::soft_delete(self.pool, user_id, id).await?;
            if !deleted {
                return Err(AppError::NotFound(format!(
                    "Lançamento com ID '{id}' não encontrado."
                )));
            }
        }
        Ok(())
    }

    pub async fn restore_transaction(
        &self,
        user_id: UserId,
        id: TransactionId,
        restore_group: bool,
    ) -> Result<u64, AppError> {
        let tx = TransactionRepository::find_by_id_including_deleted(self.pool, user_id, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Lançamento com ID '{id}' não encontrado."))
            })?;

        if !tx.is_deleted() {
            return Err(AppError::Validation(format!(
                "O lançamento com ID '{id}' não está excluído."
            )));
        }

        if restore_group {
            if let Some(group_id) = tx.installment_group_id {
                let count = TransactionRepository::restore_by_installment_group(
                    self.pool, user_id, group_id,
                )
                .await?;
                return Ok(count);
            }
        }

        if let Some(transfer_id) = tx.transfer_id {
            let count =
                TransactionRepository::restore_by_transfer_id(self.pool, user_id, transfer_id)
                    .await?;
            return Ok(count);
        }

        let restored = TransactionRepository::restore(self.pool, user_id, id).await?;
        if restored {
            Ok(1)
        } else {
            Err(AppError::NotFound(format!(
                "Lançamento com ID '{id}' não encontrado."
            )))
        }
    }

    pub async fn purge_deleted(
        &self,
        user_id: UserId,
        older_than_str: &str,
    ) -> Result<PurgeSummary, AppError> {
        let duration = parse_duration(older_than_str)?;
        let cutoff = Utc::now() - duration;
        let summary = TransactionRepository::purge_older_than(self.pool, user_id, cutoff).await?;
        Ok(summary)
    }
}

pub fn parse_duration(s: &str) -> Result<chrono::Duration, AppError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(AppError::Validation(
            "Duração não pode ser vazia. Exemplo: 30d, 90d, 1y, 6m.".to_string(),
        ));
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().map_err(|_| {
        AppError::Validation(format!(
            "Duração inválida '{s}'. Use formatos como 30d, 60d, 90d, 6m, 1y."
        ))
    })?;

    if num <= 0 {
        return Err(AppError::Validation(
            "O valor da duração deve ser maior que zero.".to_string(),
        ));
    }

    match unit.to_lowercase().as_str() {
        "d" => Ok(chrono::Duration::days(num)),
        "w" => Ok(chrono::Duration::weeks(num)),
        "m" => Ok(chrono::Duration::days(num * 30)),
        "y" => Ok(chrono::Duration::days(num * 365)),
        "h" => Ok(chrono::Duration::hours(num)),
        _ => Err(AppError::Validation(format!(
            "Unidade de tempo desconhecida em '{s}'. Use d (dias), w (semanas), m (meses), y (anos) ou h (horas)."
        ))),
    }
}
