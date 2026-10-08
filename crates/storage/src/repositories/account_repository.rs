use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{Account, AccountId, AccountKind, Money, UserId};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct AccountRepository;

impl AccountRepository {
    pub async fn create(pool: &PgPool, account: &Account) -> Result<(), StorageError> {
        let credit_limit_dec = account.credit_limit.map(|m| m.as_decimal());

        let mut tx = crate::db::begin_tx(pool).await?;
        let res = sqlx::query(
            r#"
            INSERT INTO accounts (id, user_id, name, kind, initial_balance, closing_day, due_day, credit_limit, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(account.id.as_uuid())
        .bind(account.user_id.as_uuid())
        .bind(&account.name)
        .bind(account.kind.as_str())
        .bind(account.initial_balance.as_decimal())
        .bind(account.closing_day.map(|d| d as i32))
        .bind(account.due_day.map(|d| d as i32))
        .bind(credit_limit_dec)
        .bind(account.created_at)
        .bind(account.updated_at)
        .execute(&mut *tx)
        .await;

        match res {
            Ok(_) => {
                tx.commit().await.map_err(StorageError::Database)?;
                Ok(())
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::UniqueViolation(format!(
                    "Já existe uma conta com o nome '{}'.",
                    account.name
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub async fn list_by_user(
        pool: &PgPool,
        user_id: UserId,
    ) -> Result<Vec<Account>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, name, kind, initial_balance, closing_day, due_day, credit_limit, created_at, updated_at, deleted_at
            FROM accounts
            WHERE user_id = $1 AND deleted_at IS NULL
            ORDER BY name ASC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut accounts = Vec::with_capacity(rows.len());
        for row in rows {
            accounts.push(map_account_row(row)?);
        }

        Ok(accounts)
    }

    pub async fn find_by_id_or_name(
        pool: &PgPool,
        user_id: UserId,
        query: &str,
    ) -> Result<Option<Account>, StorageError> {
        let is_uuid = Uuid::parse_str(query.trim()).ok();

        let row = if let Some(uuid_val) = is_uuid {
            sqlx::query(
                r#"
                SELECT id, user_id, name, kind, initial_balance, closing_day, due_day, credit_limit, created_at, updated_at, deleted_at
                FROM accounts
                WHERE user_id = $1 AND (id = $2 OR LOWER(name) = LOWER($3)) AND deleted_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(uuid_val)
            .bind(query.trim())
            .fetch_optional(pool)
            .await
            .map_err(StorageError::Database)?
        } else {
            sqlx::query(
                r#"
                SELECT id, user_id, name, kind, initial_balance, closing_day, due_day, credit_limit, created_at, updated_at, deleted_at
                FROM accounts
                WHERE user_id = $1 AND LOWER(name) = LOWER($2) AND deleted_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(query.trim())
            .fetch_optional(pool)
            .await
            .map_err(StorageError::Database)?
        };

        if let Some(row) = row {
            Ok(Some(map_account_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn soft_delete(
        pool: &PgPool,
        user_id: UserId,
        id: AccountId,
    ) -> Result<bool, StorageError> {
        let mut tx = crate::db::begin_tx(pool).await?;
        let res = sqlx::query(
            r#"
            UPDATE accounts
            SET deleted_at = NOW(), updated_at = NOW()
            WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(StorageError::Database)?;

        tx.commit().await.map_err(StorageError::Database)?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn has_active_transactions(
        pool: &PgPool,
        user_id: UserId,
        id: AccountId,
    ) -> Result<bool, StorageError> {
        let has_active: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM transactions
                WHERE user_id = $1 AND account_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .fetch_one(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(has_active)
    }
}

fn map_account_row(row: sqlx::postgres::PgRow) -> Result<Account, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let name: String = row.try_get("name").map_err(StorageError::Database)?;
    let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
    let balance_dec: Decimal = row
        .try_get("initial_balance")
        .map_err(StorageError::Database)?;
    let closing_day: Option<i32> = row.try_get("closing_day").map_err(StorageError::Database)?;
    let due_day: Option<i32> = row.try_get("due_day").map_err(StorageError::Database)?;
    let credit_limit_dec: Option<Decimal> = row
        .try_get("credit_limit")
        .map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(StorageError::Database)?;

    let kind: AccountKind = kind_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let initial_balance = Money::from_decimal_non_negative(balance_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let credit_limit = credit_limit_dec
        .map(Money::from_decimal_non_negative)
        .transpose()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(Account {
        id: AccountId::new(id),
        user_id: UserId::new(u_id),
        name,
        kind,
        initial_balance,
        closing_day: closing_day.map(|d| d as u8),
        due_day: due_day.map(|d| d as u8),
        credit_limit,
        created_at,
        updated_at,
        deleted_at,
    })
}
