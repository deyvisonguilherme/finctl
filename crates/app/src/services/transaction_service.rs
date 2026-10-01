use crate::errors::AppError;
use chrono::NaiveDate;
use domain::{Money, Transaction, TransactionKind, UserId};
use sqlx::PgPool;
use storage::{AccountRepository, CategoryRepository, TransactionRepository};

pub struct CreateTransactionInput {
    pub user_id: UserId,
    pub account_query: String,
    pub category_query: String,
    pub kind: TransactionKind,
    pub amount: Money,
    pub date: NaiveDate,
    pub description: String,
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
}
