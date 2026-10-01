use crate::errors::StorageError;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    AccountId, CategoryId, Money, RecurringFrequency, RecurringRule, RecurringRuleId,
    TransactionKind, UserId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringRuleDetails {
    pub id: RecurringRuleId,
    pub user_id: UserId,
    pub account_id: AccountId,
    pub account_name: String,
    pub category_id: CategoryId,
    pub category_name: String,
    pub kind: TransactionKind,
    pub amount: Money,
    pub description: String,
    pub frequency: RecurringFrequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub active: bool,
    pub last_generated_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct RecurringRepository;

impl RecurringRepository {
    pub async fn create(pool: &PgPool, rule: &RecurringRule) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO recurring_rules (
                id, user_id, account_id, category_id, kind, amount, description,
                frequency, day_of_month, day_of_week, start_date, end_date, active,
                last_generated_date, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(rule.id.as_uuid())
        .bind(rule.user_id.as_uuid())
        .bind(rule.account_id.as_uuid())
        .bind(rule.category_id.as_uuid())
        .bind(rule.kind.as_str())
        .bind(rule.amount.as_decimal())
        .bind(&rule.description)
        .bind(rule.frequency.as_str())
        .bind(rule.day_of_month.map(|d| d as i32))
        .bind(rule.day_of_week.map(|d| d as i32))
        .bind(rule.start_date)
        .bind(rule.end_date)
        .bind(rule.active)
        .bind(rule.last_generated_date)
        .bind(rule.created_at)
        .bind(rule.updated_at)
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(())
    }

    pub async fn list_by_user(
        pool: &PgPool,
        user_id: UserId,
        active_only: Option<bool>,
    ) -> Result<Vec<RecurringRuleDetails>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT 
                r.id, r.user_id, r.account_id, a.name AS account_name,
                r.category_id, c.name AS category_name,
                r.kind, r.amount, r.description, r.frequency,
                r.day_of_month, r.day_of_week, r.start_date, r.end_date,
                r.active, r.last_generated_date, r.created_at, r.updated_at
            FROM recurring_rules r
            JOIN accounts a ON r.account_id = a.id
            JOIN categories c ON r.category_id = c.id
            WHERE r.user_id = $1 AND ($2::BOOLEAN IS NULL OR r.active = $2)
            ORDER BY r.created_at DESC
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(active_only)
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_recurring_details_row(row)?);
        }
        Ok(results)
    }

    pub async fn find_by_id(
        pool: &PgPool,
        user_id: UserId,
        id: RecurringRuleId,
    ) -> Result<Option<RecurringRule>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, user_id, account_id, category_id, kind, amount, description,
                frequency, day_of_month, day_of_week, start_date, end_date, active,
                last_generated_date, created_at, updated_at
            FROM recurring_rules
            WHERE user_id = $1 AND id = $2
            LIMIT 1
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(r) = row {
            Ok(Some(map_recurring_row(r)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update(pool: &PgPool, rule: &RecurringRule) -> Result<(), StorageError> {
        let res = sqlx::query(
            r#"
            UPDATE recurring_rules
            SET account_id = $1, category_id = $2, kind = $3, amount = $4, description = $5,
                frequency = $6, day_of_month = $7, day_of_week = $8, start_date = $9, end_date = $10,
                active = $11, last_generated_date = $12, updated_at = $13
            WHERE id = $14 AND user_id = $15
            "#,
        )
        .bind(rule.account_id.as_uuid())
        .bind(rule.category_id.as_uuid())
        .bind(rule.kind.as_str())
        .bind(rule.amount.as_decimal())
        .bind(&rule.description)
        .bind(rule.frequency.as_str())
        .bind(rule.day_of_month.map(|d| d as i32))
        .bind(rule.day_of_week.map(|d| d as i32))
        .bind(rule.start_date)
        .bind(rule.end_date)
        .bind(rule.active)
        .bind(rule.last_generated_date)
        .bind(rule.updated_at)
        .bind(rule.id.as_uuid())
        .bind(rule.user_id.as_uuid())
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        if res.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!(
                "Regra de recorrência com ID '{}' não encontrada.",
                rule.id
            )));
        }

        Ok(())
    }

    pub async fn delete(
        pool: &PgPool,
        user_id: UserId,
        id: RecurringRuleId,
    ) -> Result<bool, StorageError> {
        let res = sqlx::query(
            r#"
            DELETE FROM recurring_rules
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

fn map_recurring_row(row: sqlx::postgres::PgRow) -> Result<RecurringRule, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let user_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let account_id: Uuid = row.try_get("account_id").map_err(StorageError::Database)?;
    let category_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
    let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
    let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
    let description: String = row.try_get("description").map_err(StorageError::Database)?;
    let freq_str: String = row.try_get("frequency").map_err(StorageError::Database)?;
    let day_of_month: Option<i32> = row
        .try_get("day_of_month")
        .map_err(StorageError::Database)?;
    let day_of_week: Option<i32> = row.try_get("day_of_week").map_err(StorageError::Database)?;
    let start_date: NaiveDate = row.try_get("start_date").map_err(StorageError::Database)?;
    let end_date: Option<NaiveDate> = row.try_get("end_date").map_err(StorageError::Database)?;
    let active: bool = row.try_get("active").map_err(StorageError::Database)?;
    let last_generated_date: Option<NaiveDate> = row
        .try_get("last_generated_date")
        .map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;

    let kind: TransactionKind = kind_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let amount = Money::new(amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let frequency: RecurringFrequency = freq_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(RecurringRule {
        id: RecurringRuleId::new(id),
        user_id: UserId::new(user_id),
        account_id: AccountId::new(account_id),
        category_id: CategoryId::new(category_id),
        kind,
        amount,
        description,
        frequency,
        day_of_month: day_of_month.map(|d| d as u32),
        day_of_week: day_of_week.map(|d| d as u32),
        start_date,
        end_date,
        active,
        last_generated_date,
        created_at,
        updated_at,
    })
}

fn map_recurring_details_row(
    row: sqlx::postgres::PgRow,
) -> Result<RecurringRuleDetails, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let user_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let account_id: Uuid = row.try_get("account_id").map_err(StorageError::Database)?;
    let account_name: String = row
        .try_get("account_name")
        .map_err(StorageError::Database)?;
    let category_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
    let category_name: String = row
        .try_get("category_name")
        .map_err(StorageError::Database)?;
    let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
    let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
    let description: String = row.try_get("description").map_err(StorageError::Database)?;
    let freq_str: String = row.try_get("frequency").map_err(StorageError::Database)?;
    let day_of_month: Option<i32> = row
        .try_get("day_of_month")
        .map_err(StorageError::Database)?;
    let day_of_week: Option<i32> = row.try_get("day_of_week").map_err(StorageError::Database)?;
    let start_date: NaiveDate = row.try_get("start_date").map_err(StorageError::Database)?;
    let end_date: Option<NaiveDate> = row.try_get("end_date").map_err(StorageError::Database)?;
    let active: bool = row.try_get("active").map_err(StorageError::Database)?;
    let last_generated_date: Option<NaiveDate> = row
        .try_get("last_generated_date")
        .map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;

    let kind: TransactionKind = kind_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let amount = Money::new(amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let frequency: RecurringFrequency = freq_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(RecurringRuleDetails {
        id: RecurringRuleId::new(id),
        user_id: UserId::new(user_id),
        account_id: AccountId::new(account_id),
        account_name,
        category_id: CategoryId::new(category_id),
        category_name,
        kind,
        amount,
        description,
        frequency,
        day_of_month: day_of_month.map(|d| d as u32),
        day_of_week: day_of_week.map(|d| d as u32),
        start_date,
        end_date,
        active,
        last_generated_date,
        created_at,
        updated_at,
    })
}
