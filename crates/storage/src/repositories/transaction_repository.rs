use crate::errors::StorageError;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{AccountId, CategoryId, Money, Transaction, TransactionId, TransactionKind, UserId};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct TransactionRepository;

impl TransactionRepository {
    pub async fn create(pool: &PgPool, tx: &Transaction) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO transactions (id, user_id, account_id, category_id, kind, amount, date, description, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(tx.id.as_uuid())
        .bind(tx.user_id.as_uuid())
        .bind(tx.account_id.as_uuid())
        .bind(tx.category_id.as_uuid())
        .bind(tx.kind.as_str())
        .bind(tx.amount.as_decimal())
        .bind(tx.date)
        .bind(&tx.description)
        .bind(tx.created_at)
        .bind(tx.updated_at)
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(())
    }

    pub async fn find_by_id(
        pool: &PgPool,
        user_id: UserId,
        id: TransactionId,
    ) -> Result<Option<Transaction>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, account_id, category_id, kind, amount, date, description, created_at, updated_at
            FROM transactions
            WHERE user_id = $1 AND id = $2
            LIMIT 1
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(row) = row {
            let tx_id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
            let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
            let acc_id: Uuid = row.try_get("account_id").map_err(StorageError::Database)?;
            let cat_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
            let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
            let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
            let date: NaiveDate = row.try_get("date").map_err(StorageError::Database)?;
            let description: String = row.try_get("description").map_err(StorageError::Database)?;
            let created_at: DateTime<Utc> =
                row.try_get("created_at").map_err(StorageError::Database)?;
            let updated_at: DateTime<Utc> =
                row.try_get("updated_at").map_err(StorageError::Database)?;

            let kind: TransactionKind = kind_str
                .parse()
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
            let amount = Money::new(amount_dec)
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

            Ok(Some(Transaction {
                id: TransactionId::new(tx_id),
                user_id: UserId::new(u_id),
                account_id: AccountId::new(acc_id),
                category_id: CategoryId::new(cat_id),
                kind,
                amount,
                date,
                description,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update(pool: &PgPool, tx: &Transaction) -> Result<(), StorageError> {
        let res = sqlx::query(
            r#"
            UPDATE transactions
            SET account_id = $1, category_id = $2, kind = $3, amount = $4, date = $5, description = $6, updated_at = $7
            WHERE id = $8 AND user_id = $9
            "#,
        )
        .bind(tx.account_id.as_uuid())
        .bind(tx.category_id.as_uuid())
        .bind(tx.kind.as_str())
        .bind(tx.amount.as_decimal())
        .bind(tx.date)
        .bind(&tx.description)
        .bind(tx.updated_at)
        .bind(tx.id.as_uuid())
        .bind(tx.user_id.as_uuid())
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        if res.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!(
                "Lançamento com ID '{}' não encontrado.",
                tx.id
            )));
        }

        Ok(())
    }

    pub async fn delete(
        pool: &PgPool,
        user_id: UserId,
        id: TransactionId,
    ) -> Result<bool, StorageError> {
        let res = sqlx::query("DELETE FROM transactions WHERE id = $1 AND user_id = $2")
            .bind(id.as_uuid())
            .bind(user_id.as_uuid())
            .execute(pool)
            .await
            .map_err(StorageError::Database)?;

        Ok(res.rows_affected() > 0)
    }
}
