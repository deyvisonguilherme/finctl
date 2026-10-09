use crate::errors::StorageError;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{AccountId, ContributionId, Goal, GoalContribution, GoalId, Money, UserId};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct GoalRepository;

impl GoalRepository {
    pub async fn create(pool: &PgPool, goal: &Goal) -> Result<(), StorageError> {
        let mut tx = crate::db::begin_tx(pool).await?;
        let res = sqlx::query(
            r#"
            INSERT INTO goals (id, user_id, name, target_amount, target_date, account_id, completed_at, created_at, updated_at, deleted_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(goal.id.as_uuid())
        .bind(goal.user_id.as_uuid())
        .bind(&goal.name)
        .bind(goal.target_amount.as_decimal())
        .bind(goal.target_date)
        .bind(goal.account_id.map(|a| a.as_uuid()))
        .bind(goal.completed_at)
        .bind(goal.created_at)
        .bind(goal.updated_at)
        .bind(goal.deleted_at)
        .execute(&mut *tx)
        .await;

        match res {
            Ok(_) => {
                tx.commit().await.map_err(StorageError::Database)?;
                Ok(())
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::UniqueViolation(format!(
                    "Já existe uma meta com o nome '{}'.",
                    goal.name
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub async fn update(pool: &PgPool, goal: &Goal) -> Result<(), StorageError> {
        let mut tx = crate::db::begin_tx(pool).await?;
        let res = sqlx::query(
            r#"
            UPDATE goals
            SET name = $1, target_amount = $2, target_date = $3, account_id = $4,
                completed_at = $5, updated_at = $6
            WHERE id = $7 AND user_id = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(&goal.name)
        .bind(goal.target_amount.as_decimal())
        .bind(goal.target_date)
        .bind(goal.account_id.map(|a| a.as_uuid()))
        .bind(goal.completed_at)
        .bind(goal.updated_at)
        .bind(goal.id.as_uuid())
        .bind(goal.user_id.as_uuid())
        .execute(&mut *tx)
        .await;

        match res {
            Ok(r) => {
                if r.rows_affected() == 0 {
                    return Err(StorageError::NotFound(format!(
                        "Meta com ID '{}' não encontrada.",
                        goal.id
                    )));
                }
                tx.commit().await.map_err(StorageError::Database)?;
                Ok(())
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::UniqueViolation(format!(
                    "Já existe uma meta com o nome '{}'.",
                    goal.name
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub async fn find_by_id_or_name(
        pool: &PgPool,
        user_id: UserId,
        query: &str,
    ) -> Result<Option<Goal>, StorageError> {
        let is_uuid = Uuid::parse_str(query.trim()).ok();

        let row = if let Some(uuid_val) = is_uuid {
            sqlx::query(
                r#"
                SELECT id, user_id, name, target_amount, target_date, account_id,
                       completed_at, created_at, updated_at, deleted_at
                FROM goals
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
                SELECT id, user_id, name, target_amount, target_date, account_id,
                       completed_at, created_at, updated_at, deleted_at
                FROM goals
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
            Ok(Some(map_goal_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_by_user(
        pool: &PgPool,
        user_id: UserId,
        include_completed: bool,
    ) -> Result<Vec<Goal>, StorageError> {
        let rows = if include_completed {
            sqlx::query(
                r#"
                SELECT id, user_id, name, target_amount, target_date, account_id,
                       completed_at, created_at, updated_at, deleted_at
                FROM goals
                WHERE user_id = $1 AND deleted_at IS NULL
                ORDER BY completed_at ASC NULLS FIRST, name ASC
                "#,
            )
            .bind(user_id.as_uuid())
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?
        } else {
            sqlx::query(
                r#"
                SELECT id, user_id, name, target_amount, target_date, account_id,
                       completed_at, created_at, updated_at, deleted_at
                FROM goals
                WHERE user_id = $1 AND deleted_at IS NULL AND completed_at IS NULL
                ORDER BY name ASC
                "#,
            )
            .bind(user_id.as_uuid())
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?
        };

        let mut goals = Vec::with_capacity(rows.len());
        for row in rows {
            goals.push(map_goal_row(row)?);
        }

        Ok(goals)
    }

    pub async fn soft_delete(
        pool: &PgPool,
        user_id: UserId,
        id: GoalId,
    ) -> Result<bool, StorageError> {
        let mut tx = crate::db::begin_tx(pool).await?;

        // Soft delete na meta
        let res = sqlx::query(
            r#"
            UPDATE goals
            SET deleted_at = NOW(), updated_at = NOW()
            WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(StorageError::Database)?;

        if res.rows_affected() > 0 {
            // Soft delete também nos aportes
            sqlx::query(
                r#"
                UPDATE goal_contributions
                SET deleted_at = NOW(), updated_at = NOW()
                WHERE user_id = $1 AND goal_id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(StorageError::Database)?;

            tx.commit().await.map_err(StorageError::Database)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn add_contribution(
        pool: &PgPool,
        contrib: &GoalContribution,
    ) -> Result<(), StorageError> {
        let mut tx = crate::db::begin_tx(pool).await?;
        sqlx::query(
            r#"
            INSERT INTO goal_contributions (id, goal_id, user_id, amount, date, note, created_at, updated_at, deleted_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(contrib.id.as_uuid())
        .bind(contrib.goal_id.as_uuid())
        .bind(contrib.user_id.as_uuid())
        .bind(contrib.amount.as_decimal())
        .bind(contrib.date)
        .bind(&contrib.note)
        .bind(contrib.created_at)
        .bind(contrib.updated_at)
        .bind(contrib.deleted_at)
        .execute(&mut *tx)
        .await
        .map_err(StorageError::Database)?;

        tx.commit().await.map_err(StorageError::Database)?;
        Ok(())
    }

    pub async fn list_contributions(
        pool: &PgPool,
        user_id: UserId,
        goal_id: GoalId,
    ) -> Result<Vec<GoalContribution>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, goal_id, user_id, amount, date, note, created_at, updated_at, deleted_at
            FROM goal_contributions
            WHERE user_id = $1 AND goal_id = $2 AND deleted_at IS NULL
            ORDER BY date DESC, created_at DESC
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(goal_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            list.push(map_contribution_row(row)?);
        }

        Ok(list)
    }

    pub async fn get_manual_goal_sum(
        pool: &PgPool,
        user_id: UserId,
        goal_id: GoalId,
    ) -> Result<Decimal, StorageError> {
        let total: Option<Decimal> = sqlx::query_scalar(
            r#"
            SELECT SUM(amount)
            FROM goal_contributions
            WHERE user_id = $1 AND goal_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(goal_id.as_uuid())
        .fetch_one(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(total.unwrap_or(Decimal::ZERO))
    }

    pub async fn get_manual_contributions_sum_since(
        pool: &PgPool,
        user_id: UserId,
        goal_id: GoalId,
        since: NaiveDate,
    ) -> Result<Decimal, StorageError> {
        let total: Option<Decimal> = sqlx::query_scalar(
            r#"
            SELECT SUM(amount)
            FROM goal_contributions
            WHERE user_id = $1 AND goal_id = $2 AND date >= $3 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(goal_id.as_uuid())
        .bind(since)
        .fetch_one(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(total.unwrap_or(Decimal::ZERO))
    }

    pub async fn get_account_balance(
        pool: &PgPool,
        user_id: UserId,
        account_id: AccountId,
    ) -> Result<Decimal, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT 
                a.initial_balance,
                COALESCE(SUM(CASE WHEN t.kind = 'income' THEN t.amount ELSE -t.amount END), 0.00) AS net_tx
            FROM accounts a
            LEFT JOIN transactions t ON a.id = t.account_id AND t.user_id = a.user_id 
                AND t.status = 'paid' AND t.deleted_at IS NULL
            WHERE a.id = $1 AND a.user_id = $2 AND a.deleted_at IS NULL
            GROUP BY a.initial_balance
            "#,
        )
        .bind(account_id.as_uuid())
        .bind(user_id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(r) = row {
            let initial: Decimal = r
                .try_get("initial_balance")
                .map_err(StorageError::Database)?;
            let net: Decimal = r.try_get("net_tx").map_err(StorageError::Database)?;
            Ok(initial + net)
        } else {
            Ok(Decimal::ZERO)
        }
    }

    pub async fn get_account_net_flow_since(
        pool: &PgPool,
        user_id: UserId,
        account_id: AccountId,
        since: NaiveDate,
    ) -> Result<Decimal, StorageError> {
        let total: Option<Decimal> = sqlx::query_scalar(
            r#"
            SELECT SUM(CASE WHEN kind = 'income' THEN amount ELSE -amount END)
            FROM transactions
            WHERE account_id = $1 AND user_id = $2 AND date >= $3 
              AND status = 'paid' AND deleted_at IS NULL
            "#,
        )
        .bind(account_id.as_uuid())
        .bind(user_id.as_uuid())
        .bind(since)
        .fetch_one(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(total.unwrap_or(Decimal::ZERO))
    }

    pub async fn get_earliest_contribution_date(
        pool: &PgPool,
        user_id: UserId,
        goal_id: GoalId,
    ) -> Result<Option<NaiveDate>, StorageError> {
        let date: Option<NaiveDate> = sqlx::query_scalar(
            r#"
            SELECT MIN(date)
            FROM goal_contributions
            WHERE user_id = $1 AND goal_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(goal_id.as_uuid())
        .fetch_one(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(date)
    }

    pub async fn get_account_created_at_or_first_tx(
        pool: &PgPool,
        user_id: UserId,
        account_id: AccountId,
    ) -> Result<Option<NaiveDate>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT a.created_at::DATE AS acc_date, MIN(t.date) AS tx_date
            FROM accounts a
            LEFT JOIN transactions t ON a.id = t.account_id AND t.user_id = a.user_id 
                AND t.status = 'paid' AND t.deleted_at IS NULL
            WHERE a.id = $1 AND a.user_id = $2 AND a.deleted_at IS NULL
            GROUP BY a.created_at
            "#,
        )
        .bind(account_id.as_uuid())
        .bind(user_id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(r) = row {
            let acc_date: NaiveDate = r.try_get("acc_date").map_err(StorageError::Database)?;
            let tx_date: Option<NaiveDate> =
                r.try_get("tx_date").map_err(StorageError::Database)?;
            let earliest = match tx_date {
                Some(td) => td.min(acc_date),
                None => acc_date,
            };
            Ok(Some(earliest))
        } else {
            Ok(None)
        }
    }
}

fn map_goal_row(row: sqlx::postgres::PgRow) -> Result<Goal, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let name: String = row.try_get("name").map_err(StorageError::Database)?;
    let target_amount_dec: Decimal = row
        .try_get("target_amount")
        .map_err(StorageError::Database)?;
    let target_date: Option<NaiveDate> =
        row.try_get("target_date").map_err(StorageError::Database)?;
    let account_id_uuid: Option<Uuid> =
        row.try_get("account_id").map_err(StorageError::Database)?;
    let completed_at: Option<DateTime<Utc>> = row
        .try_get("completed_at")
        .map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(StorageError::Database)?;

    let target_amount = Money::from_decimal_non_negative(target_amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(Goal {
        id: GoalId::new(id),
        user_id: UserId::new(u_id),
        name,
        target_amount,
        target_date,
        account_id: account_id_uuid.map(AccountId::new),
        completed_at,
        created_at,
        updated_at,
        deleted_at,
    })
}

fn map_contribution_row(row: sqlx::postgres::PgRow) -> Result<GoalContribution, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let goal_id_uuid: Uuid = row.try_get("goal_id").map_err(StorageError::Database)?;
    let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
    let date: NaiveDate = row.try_get("date").map_err(StorageError::Database)?;
    let note: Option<String> = row.try_get("note").map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(StorageError::Database)?;

    let amount = Money::from_decimal_non_negative(amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(GoalContribution {
        id: ContributionId::new(id),
        goal_id: GoalId::new(goal_id_uuid),
        user_id: UserId::new(u_id),
        amount,
        date,
        note,
        created_at,
        updated_at,
        deleted_at,
    })
}
