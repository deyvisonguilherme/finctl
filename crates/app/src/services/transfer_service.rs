use crate::errors::AppError;
use chrono::NaiveDate;
use domain::{Money, Transaction, TransactionKind, TransactionStatus, UserId};
use sqlx::PgPool;
use storage::{AccountRepository, CategoryRepository, TransactionRepository};
use uuid::Uuid;

pub struct CreateTransferInput {
    pub user_id: UserId,
    pub from_account_query: String,
    pub to_account_query: String,
    pub amount: Money,
    pub date: NaiveDate,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateTransferSummary {
    pub transfer_id: Uuid,
    pub from_account_name: String,
    pub to_account_name: String,
    pub amount: Money,
    pub date: NaiveDate,
    pub from_transaction: Transaction,
    pub to_transaction: Transaction,
}

pub struct TransferService<'a> {
    pool: &'a PgPool,
}

impl<'a> TransferService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_transfer(
        &self,
        input: CreateTransferInput,
    ) -> Result<CreateTransferSummary, AppError> {
        let from_account = AccountRepository::find_by_id_or_name(
            self.pool,
            input.user_id,
            &input.from_account_query,
        )
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Conta de origem '{}' não encontrada.",
                input.from_account_query
            ))
        })?;

        let to_account = AccountRepository::find_by_id_or_name(
            self.pool,
            input.user_id,
            &input.to_account_query,
        )
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Conta de destino '{}' não encontrada.",
                input.to_account_query
            ))
        })?;

        if from_account.id == to_account.id {
            return Err(AppError::Validation(
                "As contas de origem e destino devem ser diferentes.".to_string(),
            ));
        }

        let expense_cat = CategoryRepository::get_or_create_transfer_category(
            self.pool,
            input.user_id,
            TransactionKind::Expense,
        )
        .await?;

        let income_cat = CategoryRepository::get_or_create_transfer_category(
            self.pool,
            input.user_id,
            TransactionKind::Income,
        )
        .await?;

        let transfer_id = Uuid::new_v4();

        let from_desc = match input.description {
            Some(ref d) if !d.trim().is_empty() => d.trim().to_string(),
            _ => format!("Transferência para {}", to_account.name),
        };

        let to_desc = match input.description {
            Some(ref d) if !d.trim().is_empty() => d.trim().to_string(),
            _ => format!("Transferência de {}", from_account.name),
        };

        let from_tx = Transaction::new_full(
            input.user_id,
            from_account.id,
            expense_cat.id,
            TransactionKind::Expense,
            input.amount,
            input.date,
            from_desc,
            TransactionStatus::Paid,
            Some(transfer_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )?;

        let to_tx = Transaction::new_full(
            input.user_id,
            to_account.id,
            income_cat.id,
            TransactionKind::Income,
            input.amount,
            input.date,
            to_desc,
            TransactionStatus::Paid,
            Some(transfer_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )?;

        TransactionRepository::create_batch(self.pool, &[from_tx.clone(), to_tx.clone()]).await?;

        Ok(CreateTransferSummary {
            transfer_id,
            from_account_name: from_account.name,
            to_account_name: to_account.name,
            amount: input.amount,
            date: input.date,
            from_transaction: from_tx,
            to_transaction: to_tx,
        })
    }
}
