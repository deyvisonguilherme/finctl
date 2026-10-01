use crate::errors::AppError;
use chrono::NaiveDate;
use domain::{AccountId, AccountKind, Money, UserId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account_id: AccountId,
    pub account_name: String,
    pub account_kind: AccountKind,
    pub initial_balance: Money,
    pub total_income: Money,
    pub total_expense: Money,
    pub current_balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceReport {
    pub as_of_date: Option<NaiveDate>,
    pub accounts: Vec<AccountBalance>,
    pub total_initial_balance: Decimal,
    pub total_income: Decimal,
    pub total_expense: Decimal,
    pub total_balance: Decimal,
}

pub struct BalanceService<'a> {
    pool: &'a PgPool,
}

impl<'a> BalanceService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_balance(
        &self,
        user_id: UserId,
        at_date: Option<NaiveDate>,
    ) -> Result<BalanceReport, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id AS account_id,
                a.name AS account_name,
                a.kind AS account_kind,
                a.initial_balance,
                COALESCE(SUM(CASE WHEN t.kind = 'income' THEN t.amount ELSE 0 END), 0.00) AS total_income,
                COALESCE(SUM(CASE WHEN t.kind = 'expense' THEN t.amount ELSE 0 END), 0.00) AS total_expense
            FROM accounts a
            LEFT JOIN transactions t ON a.id = t.account_id AND t.user_id = a.user_id 
                AND ($2::DATE IS NULL OR t.date <= $2)
            WHERE a.user_id = $1
            GROUP BY a.id, a.name, a.kind, a.initial_balance
            ORDER BY a.name ASC
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(at_date)
        .fetch_all(self.pool)
        .await
        .map_err(storage::StorageError::Database)?;

        let mut accounts = Vec::with_capacity(rows.len());
        let mut sum_initial = Decimal::ZERO;
        let mut sum_income = Decimal::ZERO;
        let mut sum_expense = Decimal::ZERO;
        let mut sum_total = Decimal::ZERO;

        for row in rows {
            let acc_id: Uuid = row
                .try_get("account_id")
                .map_err(storage::StorageError::Database)?;
            let acc_name: String = row
                .try_get("account_name")
                .map_err(storage::StorageError::Database)?;
            let kind_str: String = row
                .try_get("account_kind")
                .map_err(storage::StorageError::Database)?;
            let initial_dec: Decimal = row
                .try_get("initial_balance")
                .map_err(storage::StorageError::Database)?;
            let income_dec: Decimal = row
                .try_get("total_income")
                .map_err(storage::StorageError::Database)?;
            let expense_dec: Decimal = row
                .try_get("total_expense")
                .map_err(storage::StorageError::Database)?;

            let kind: AccountKind = kind_str
                .parse()
                .map_err(|e| storage::StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
            let initial_balance = Money::from_decimal_non_negative(initial_dec)
                .map_err(|e| storage::StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
            let total_income = Money::from_decimal_non_negative(income_dec)
                .map_err(|e| storage::StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;
            let total_expense = Money::from_decimal_non_negative(expense_dec)
                .map_err(|e| storage::StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

            let current_balance = initial_dec + income_dec - expense_dec;

            sum_initial += initial_dec;
            sum_income += income_dec;
            sum_expense += expense_dec;
            sum_total += current_balance;

            accounts.push(AccountBalance {
                account_id: AccountId::new(acc_id),
                account_name: acc_name,
                account_kind: kind,
                initial_balance,
                total_income,
                total_expense,
                current_balance,
            });
        }

        Ok(BalanceReport {
            as_of_date: at_date,
            accounts,
            total_initial_balance: sum_initial,
            total_income: sum_income,
            total_expense: sum_expense,
            total_balance: sum_total,
        })
    }
}
