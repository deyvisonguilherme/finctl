use crate::errors::AppError;
use chrono::{Days, Local, NaiveDate, Utc};
use domain::{Money, Transaction, TransactionId, TransactionKind, UserId};
use sqlx::PgPool;
use storage::{
    AccountRepository, CategoryRepository, TransactionDetails, TransactionFilter,
    TransactionRepository,
};

pub struct CreateTransactionInput {
    pub user_id: UserId,
    pub account_query: String,
    pub category_query: String,
    pub kind: TransactionKind,
    pub amount: Money,
    pub date: NaiveDate,
    pub description: String,
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

        let transaction = Transaction::new(
            input.user_id,
            account.id,
            category.id,
            input.kind,
            input.amount,
            input.date,
            input.description,
        )?;

        TransactionRepository::create(self.pool, &transaction).await?;
        Ok(transaction)
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
            limit: input.limit,
            ..Default::default()
        };

        let transactions = TransactionRepository::list_with_details(self.pool, filter).await?;
        Ok(transactions)
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
