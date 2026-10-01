use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{Account, AccountId, AccountKind, Money, UserId};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct AccountRepository;

impl AccountRepository {
    pub async fn create(pool: &PgPool, account: &Account) -> Result<(), StorageError> {
        let res = sqlx::query(
            r#"
            INSERT INTO accounts (id, user_id, name, kind, initial_balance, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(account.id.as_uuid())
        .bind(account.user_id.as_uuid())
        .bind(&account.name)
        .bind(account.kind.as_str())
        .bind(account.initial_balance.as_decimal())
        .bind(account.created_at)
        .bind(account.updated_at)
        .execute(pool)
        .await;

        match res {
            Ok(_) => Ok(()),
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
            SELECT id, user_id, name, kind, initial_balance, created_at, updated_at
            FROM accounts
            WHERE user_id = $1
            ORDER BY name ASC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut accounts = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
            let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
            let name: String = row.try_get("name").map_err(StorageError::Database)?;
            let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
            let balance_dec: Decimal = row
                .try_get("initial_balance")
                .map_err(StorageError::Database)?;
            let created_at: DateTime<Utc> =
                row.try_get("created_at").map_err(StorageError::Database)?;
            let updated_at: DateTime<Utc> =
                row.try_get("updated_at").map_err(StorageError::Database)?;

            let kind: AccountKind = kind_str
                .parse()
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
            let initial_balance = Money::from_decimal_non_negative(balance_dec)
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

            accounts.push(Account {
                id: AccountId::new(id),
                user_id: UserId::new(u_id),
                name,
                kind,
                initial_balance,
                created_at,
                updated_at,
            });
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
                SELECT id, user_id, name, kind, initial_balance, created_at, updated_at
                FROM accounts
                WHERE user_id = $1 AND (id = $2 OR LOWER(name) = LOWER($3))
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
                SELECT id, user_id, name, kind, initial_balance, created_at, updated_at
                FROM accounts
                WHERE user_id = $1 AND LOWER(name) = LOWER($2)
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
            let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
            let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
            let name: String = row.try_get("name").map_err(StorageError::Database)?;
            let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
            let balance_dec: Decimal = row
                .try_get("initial_balance")
                .map_err(StorageError::Database)?;
            let created_at: DateTime<Utc> =
                row.try_get("created_at").map_err(StorageError::Database)?;
            let updated_at: DateTime<Utc> =
                row.try_get("updated_at").map_err(StorageError::Database)?;

            let kind: AccountKind = kind_str
                .parse()
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
            let initial_balance = Money::from_decimal_non_negative(balance_dec)
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

            Ok(Some(Account {
                id: AccountId::new(id),
                user_id: UserId::new(u_id),
                name,
                kind,
                initial_balance,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }
}
