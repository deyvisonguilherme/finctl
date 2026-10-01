use crate::errors::StorageError;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    AccountId, CategoryId, Money, Transaction, TransactionId, TransactionKind, TransactionStatus,
    UserId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    pub id: TransactionId,
    pub user_id: UserId,
    pub account_id: AccountId,
    pub account_name: String,
    pub category_id: CategoryId,
    pub category_name: String,
    pub kind: TransactionKind,
    pub amount: Money,
    pub date: NaiveDate,
    pub description: String,
    pub status: TransactionStatus,
    pub transfer_id: Option<Uuid>,
    pub installment_group_id: Option<Uuid>,
    pub installment_number: Option<u32>,
    pub installment_total: Option<u32>,
    pub recurring_rule_id: Option<Uuid>,
    pub import_hash: Option<String>,
    pub reconciled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Default, Clone)]
pub struct TransactionFilter {
    pub user_id: UserId,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub account_id: Option<AccountId>,
    pub category_id: Option<CategoryId>,
    pub kind: Option<TransactionKind>,
    pub status: Option<TransactionStatus>,
    pub include_transfers: bool,
    pub transfer_id: Option<Uuid>,
    pub installment_group_id: Option<Uuid>,
    pub recurring_rule_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub struct TransactionRepository;

impl TransactionRepository {
    pub async fn create(pool: &PgPool, tx: &Transaction) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO transactions (
                id, user_id, account_id, category_id, kind, amount, date, description,
                status, transfer_id, installment_group_id, installment_number, installment_total,
                recurring_rule_id, import_hash, reconciled_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
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
        .bind(tx.status.as_str())
        .bind(tx.transfer_id)
        .bind(tx.installment_group_id)
        .bind(tx.installment_number.map(|n| n as i32))
        .bind(tx.installment_total.map(|t| t as i32))
        .bind(tx.recurring_rule_id)
        .bind(&tx.import_hash)
        .bind(tx.reconciled_at)
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
            SELECT 
                id, user_id, account_id, category_id, kind, amount, date, description,
                status, transfer_id, installment_group_id, installment_number, installment_total,
                recurring_rule_id, import_hash, reconciled_at, created_at, updated_at
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
            Ok(Some(map_transaction_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_with_details(
        pool: &PgPool,
        filter: TransactionFilter,
    ) -> Result<Vec<TransactionDetails>, StorageError> {
        let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"
            SELECT 
                t.id, t.user_id, t.account_id, a.name AS account_name,
                t.category_id, c.name AS category_name,
                t.kind, t.amount, t.date, t.description,
                t.status, t.transfer_id, t.installment_group_id,
                t.installment_number, t.installment_total, t.recurring_rule_id,
                t.import_hash, t.reconciled_at,
                t.created_at, t.updated_at
            FROM transactions t
            JOIN accounts a ON t.account_id = a.id
            JOIN categories c ON t.category_id = c.id
            WHERE t.user_id = 
            "#,
        );
        builder.push_bind(filter.user_id.as_uuid());

        if let Some(from) = filter.from_date {
            builder.push(" AND t.date >= ");
            builder.push_bind(from);
        }

        if let Some(to) = filter.to_date {
            builder.push(" AND t.date <= ");
            builder.push_bind(to);
        }

        if let Some(acc_id) = filter.account_id {
            builder.push(" AND t.account_id = ");
            builder.push_bind(acc_id.as_uuid());
        }

        if let Some(cat_id) = filter.category_id {
            builder.push(" AND t.category_id = ");
            builder.push_bind(cat_id.as_uuid());
        }

        if let Some(kind) = filter.kind {
            builder.push(" AND t.kind = ");
            builder.push_bind(kind.as_str());
        }

        if let Some(status) = filter.status {
            builder.push(" AND t.status = ");
            builder.push_bind(status.as_str());
        }

        if !filter.include_transfers && filter.transfer_id.is_none() {
            // Default filter behavior: if include_transfers is explicitly false, include all unless excluded in reports
        }

        if let Some(transfer_id) = filter.transfer_id {
            builder.push(" AND t.transfer_id = ");
            builder.push_bind(transfer_id);
        }

        if let Some(group_id) = filter.installment_group_id {
            builder.push(" AND t.installment_group_id = ");
            builder.push_bind(group_id);
        }

        if let Some(rule_id) = filter.recurring_rule_id {
            builder.push(" AND t.recurring_rule_id = ");
            builder.push_bind(rule_id);
        }

        builder.push(" ORDER BY t.date DESC, t.created_at DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ");
            builder.push_bind(limit);
        }

        let query = builder.build();
        let rows = query
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_transaction_details_row(row)?);
        }

        Ok(results)
    }

    pub async fn update(pool: &PgPool, tx: &Transaction) -> Result<(), StorageError> {
        let res = sqlx::query(
            r#"
            UPDATE transactions
            SET account_id = $1, category_id = $2, kind = $3, amount = $4, date = $5, description = $6,
                status = $7, transfer_id = $8, installment_group_id = $9, installment_number = $10,
                installment_total = $11, recurring_rule_id = $12, import_hash = $13, reconciled_at = $14,
                updated_at = $15
            WHERE id = $16 AND user_id = $17
            "#,
        )
        .bind(tx.account_id.as_uuid())
        .bind(tx.category_id.as_uuid())
        .bind(tx.kind.as_str())
        .bind(tx.amount.as_decimal())
        .bind(tx.date)
        .bind(&tx.description)
        .bind(tx.status.as_str())
        .bind(tx.transfer_id)
        .bind(tx.installment_group_id)
        .bind(tx.installment_number.map(|n| n as i32))
        .bind(tx.installment_total.map(|t| t as i32))
        .bind(tx.recurring_rule_id)
        .bind(&tx.import_hash)
        .bind(tx.reconciled_at)
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

fn map_transaction_row(row: sqlx::postgres::PgRow) -> Result<Transaction, StorageError> {
    let tx_id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let acc_id: Uuid = row.try_get("account_id").map_err(StorageError::Database)?;
    let cat_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
    let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
    let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
    let date: NaiveDate = row.try_get("date").map_err(StorageError::Database)?;
    let description: String = row.try_get("description").map_err(StorageError::Database)?;
    let status_str: String = row.try_get("status").unwrap_or_else(|_| "paid".to_string());
    let transfer_id: Option<Uuid> = row.try_get("transfer_id").ok().flatten();
    let installment_group_id: Option<Uuid> = row.try_get("installment_group_id").ok().flatten();
    let installment_number: Option<i32> = row.try_get("installment_number").ok().flatten();
    let installment_total: Option<i32> = row.try_get("installment_total").ok().flatten();
    let recurring_rule_id: Option<Uuid> = row.try_get("recurring_rule_id").ok().flatten();
    let import_hash: Option<String> = row.try_get("import_hash").ok().flatten();
    let reconciled_at: Option<DateTime<Utc>> = row.try_get("reconciled_at").ok().flatten();
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;

    let kind: TransactionKind = kind_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let status: TransactionStatus = status_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let amount = Money::new(amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(Transaction {
        id: TransactionId::new(tx_id),
        user_id: UserId::new(u_id),
        account_id: AccountId::new(acc_id),
        category_id: CategoryId::new(cat_id),
        kind,
        amount,
        date,
        description,
        status,
        transfer_id,
        installment_group_id,
        installment_number: installment_number.map(|n| n as u32),
        installment_total: installment_total.map(|t| t as u32),
        recurring_rule_id,
        import_hash,
        reconciled_at,
        created_at,
        updated_at,
    })
}

fn map_transaction_details_row(
    row: sqlx::postgres::PgRow,
) -> Result<TransactionDetails, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let acc_id: Uuid = row.try_get("account_id").map_err(StorageError::Database)?;
    let account_name: String = row
        .try_get("account_name")
        .map_err(StorageError::Database)?;
    let cat_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
    let category_name: String = row
        .try_get("category_name")
        .map_err(StorageError::Database)?;
    let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
    let amount_dec: Decimal = row.try_get("amount").map_err(StorageError::Database)?;
    let date: NaiveDate = row.try_get("date").map_err(StorageError::Database)?;
    let description: String = row.try_get("description").map_err(StorageError::Database)?;
    let status_str: String = row.try_get("status").unwrap_or_else(|_| "paid".to_string());
    let transfer_id: Option<Uuid> = row.try_get("transfer_id").ok().flatten();
    let installment_group_id: Option<Uuid> = row.try_get("installment_group_id").ok().flatten();
    let installment_number: Option<i32> = row.try_get("installment_number").ok().flatten();
    let installment_total: Option<i32> = row.try_get("installment_total").ok().flatten();
    let recurring_rule_id: Option<Uuid> = row.try_get("recurring_rule_id").ok().flatten();
    let import_hash: Option<String> = row.try_get("import_hash").ok().flatten();
    let reconciled_at: Option<DateTime<Utc>> = row.try_get("reconciled_at").ok().flatten();
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;

    let kind: TransactionKind = kind_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let status: TransactionStatus = status_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
    let amount = Money::new(amount_dec)
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(TransactionDetails {
        id: TransactionId::new(id),
        user_id: UserId::new(u_id),
        account_id: AccountId::new(acc_id),
        account_name,
        category_id: CategoryId::new(cat_id),
        category_name,
        kind,
        amount,
        date,
        description,
        status,
        transfer_id,
        installment_group_id,
        installment_number: installment_number.map(|n| n as u32),
        installment_total: installment_total.map(|t| t as u32),
        recurring_rule_id,
        import_hash,
        reconciled_at,
        created_at,
        updated_at,
    })
}
