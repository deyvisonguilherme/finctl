use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{Budget, BudgetId, CategoryId, Money, UserId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetDetails {
    pub id: BudgetId,
    pub user_id: UserId,
    pub category_id: CategoryId,
    pub category_name: String,
    pub amount: Money,
    pub month: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct BudgetRepository;

impl BudgetRepository {
    pub async fn upsert(pool: &PgPool, budget: &Budget) -> Result<(), StorageError> {
        let res = if budget.month.is_none() {
            sqlx::query(
                r#"
                INSERT INTO budgets (id, user_id, category_id, amount, month, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (user_id, category_id) WHERE month IS NULL
                DO UPDATE SET amount = EXCLUDED.amount, updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(budget.id.as_uuid())
            .bind(budget.user_id.as_uuid())
            .bind(budget.category_id.as_uuid())
            .bind(budget.amount.as_decimal())
            .bind(&budget.month)
            .bind(budget.created_at)
            .bind(budget.updated_at)
            .execute(pool)
            .await
        } else {
            sqlx::query(
                r#"
                INSERT INTO budgets (id, user_id, category_id, amount, month, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (user_id, category_id, month) WHERE month IS NOT NULL
                DO UPDATE SET amount = EXCLUDED.amount, updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(budget.id.as_uuid())
            .bind(budget.user_id.as_uuid())
            .bind(budget.category_id.as_uuid())
            .bind(budget.amount.as_decimal())
            .bind(&budget.month)
            .bind(budget.created_at)
            .bind(budget.updated_at)
            .execute(pool)
            .await
        };

        res.map(|_| ()).map_err(StorageError::Database)
    }

    pub async fn list_by_user(
        pool: &PgPool,
        user_id: UserId,
        month: Option<&str>,
    ) -> Result<Vec<BudgetDetails>, StorageError> {
        let rows = if let Some(m) = month {
            sqlx::query(
                r#"
                SELECT b.id, b.user_id, b.category_id, c.name AS category_name, b.amount, b.month, b.created_at, b.updated_at
                FROM budgets b
                JOIN categories c ON b.category_id = c.id
                WHERE b.user_id = $1 AND (b.month = $2 OR b.month IS NULL)
                ORDER BY c.name ASC, b.month DESC NULLS LAST
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(m)
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?
        } else {
            sqlx::query(
                r#"
                SELECT b.id, b.user_id, b.category_id, c.name AS category_name, b.amount, b.month, b.created_at, b.updated_at
                FROM budgets b
                JOIN categories c ON b.category_id = c.id
                WHERE b.user_id = $1
                ORDER BY c.name ASC, b.month DESC NULLS LAST
                "#,
            )
            .bind(user_id.as_uuid())
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?
        };

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_budget_details_row(row)?);
        }
        Ok(results)
    }

    pub async fn find_active_budgets_for_month(
        pool: &PgPool,
        user_id: UserId,
        month: &str,
    ) -> Result<Vec<BudgetDetails>, StorageError> {
        // Retorna o orçamento do mês específico caso exista, senão o padrão recorrente (month IS NULL)
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (b.category_id)
                b.id, b.user_id, b.category_id, c.name AS category_name, b.amount, b.month, b.created_at, b.updated_at
            FROM budgets b
            JOIN categories c ON b.category_id = c.id
            WHERE b.user_id = $1 AND (b.month = $2 OR b.month IS NULL)
            ORDER BY b.category_id, (CASE WHEN b.month = $2 THEN 0 ELSE 1 END) ASC, b.updated_at DESC
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(month)
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_budget_details_row(row)?);
        }
        Ok(results)
    }

    pub async fn find_for_category_and_month(
        pool: &PgPool,
        user_id: UserId,
        category_id: CategoryId,
        month: &str,
    ) -> Result<Option<BudgetDetails>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT b.id, b.user_id, b.category_id, c.name AS category_name, b.amount, b.month, b.created_at, b.updated_at
            FROM budgets b
            JOIN categories c ON b.category_id = c.id
            WHERE b.user_id = $1 AND b.category_id = $2 AND (b.month = $3 OR b.month IS NULL)
            ORDER BY (CASE WHEN b.month = $3 THEN 0 ELSE 1 END) ASC
            LIMIT 1
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(category_id.as_uuid())
        .bind(month)
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(r) = row {
            Ok(Some(map_budget_details_row(r)?))
        } else {
            Ok(None)
        }
    }

    pub async fn delete(
        pool: &PgPool,
        user_id: UserId,
        id: BudgetId,
    ) -> Result<bool, StorageError> {
        let res = sqlx::query(
            r#"
            DELETE FROM budgets
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(res.rows_affected() > 0)
    }
}

fn map_budget_details_row(row: sqlx::postgres::PgRow) -> Result<BudgetDetails, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let user_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let category_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
    let category_name: String = row
        .try_get("category_name")
        .map_err(StorageError::Database)?;
    let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
    let month: Option<String> = row.try_get("month").map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;

    let amount = Money::new(amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(BudgetDetails {
        id: BudgetId::new(id),
        user_id: UserId::new(user_id),
        category_id: CategoryId::new(category_id),
        category_name,
        amount,
        month,
        created_at,
        updated_at,
    })
}
