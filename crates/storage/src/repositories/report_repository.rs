use crate::errors::StorageError;
use chrono::NaiveDate;
use domain::{
    AccountId, CategoryComparisonReport, CategoryComparisonRow, CategoryId, CategoryReportItem,
    CategoryReportSummary, Money, MonthlyReportItem, TransactionKind, UserId,
};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct MonthlyReportFilter {
    pub user_id: UserId,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub account_id: Option<AccountId>,
    pub include_pending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CategoryReportFilter {
    pub user_id: UserId,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub account_id: Option<AccountId>,
    pub kind: Option<TransactionKind>,
    pub depth: u32,
    pub include_pending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CategoryCompareFilter {
    pub user_id: UserId,
    pub months: Vec<String>,
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
    pub account_id: Option<AccountId>,
    pub kind: Option<TransactionKind>,
    pub include_pending: bool,
}

pub struct ReportRepository;

impl ReportRepository {
    pub async fn monthly_summary(
        pool: &PgPool,
        filter: MonthlyReportFilter,
    ) -> Result<Vec<MonthlyReportItem>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT 
                TO_CHAR(date, 'YYYY-MM') AS month_str,
                COALESCE(SUM(CASE WHEN kind = 'income' THEN amount ELSE 0 END), 0) AS total_income,
                COALESCE(SUM(CASE WHEN kind = 'expense' THEN amount ELSE 0 END), 0) AS total_expense
            FROM transactions
            WHERE user_id = $1
              AND transfer_id IS NULL
              AND ($2::DATE IS NULL OR date >= $2)
              AND ($3::DATE IS NULL OR date <= $3)
              AND ($4::UUID IS NULL OR account_id = $4)
              AND ($5::BOOLEAN = TRUE OR status = 'paid')
            GROUP BY month_str
            ORDER BY month_str ASC
            "#,
        )
        .bind(filter.user_id.as_uuid())
        .bind(filter.from_date)
        .bind(filter.to_date)
        .bind(filter.account_id.map(|id| id.as_uuid()))
        .bind(filter.include_pending)
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let month_str: String = row.try_get("month_str").map_err(StorageError::Database)?;
            let total_income_dec: Decimal = row
                .try_get("total_income")
                .map_err(StorageError::Database)?;
            let total_expense_dec: Decimal = row
                .try_get("total_expense")
                .map_err(StorageError::Database)?;

            let total_income = Money::from_decimal_non_negative(total_income_dec).map_err(|e| {
                StorageError::Conversion(format!("Valor inválido para receita total: {e}"))
            })?;
            let total_expense =
                Money::from_decimal_non_negative(total_expense_dec).map_err(|e| {
                    StorageError::Conversion(format!("Valor inválido para despesa total: {e}"))
                })?;

            let net_balance = total_income_dec - total_expense_dec;
            let savings_rate = if total_income_dec > Decimal::ZERO {
                let rate = (net_balance / total_income_dec) * Decimal::from(100);
                rate.round_dp(2)
            } else {
                Decimal::ZERO
            };

            items.push(MonthlyReportItem {
                month: month_str,
                total_income,
                total_expense,
                net_balance,
                savings_rate,
            });
        }

        Ok(items)
    }

    pub async fn category_summary(
        pool: &PgPool,
        filter: CategoryReportFilter,
    ) -> Result<CategoryReportSummary, StorageError> {
        let depth = if filter.depth == 1 { 1 } else { 2 };
        let kind_str = filter.kind.map(|k| k.as_str().to_string());

        let rows = if depth == 1 {
            // Aggregate subcategories into parent category
            sqlx::query(
                r#"
                WITH tx_data AS (
                    SELECT 
                        t.id,
                        t.amount,
                        COALESCE(c.parent_id, c.id) AS effective_category_id
                    FROM transactions t
                    JOIN categories c ON c.id = t.category_id
                    WHERE t.user_id = $1
                      AND t.transfer_id IS NULL
                      AND ($2::DATE IS NULL OR t.date >= $2)
                      AND ($3::DATE IS NULL OR t.date <= $3)
                      AND ($4::UUID IS NULL OR t.account_id = $4)
                      AND ($5::TEXT IS NULL OR t.kind = $5)
                      AND ($6::BOOLEAN = TRUE OR t.status = 'paid')
                )
                SELECT 
                    cat.id AS category_id,
                    cat.name AS category_name,
                    NULL::UUID AS parent_id,
                    NULL::TEXT AS parent_name,
                    cat.kind AS category_kind,
                    COALESCE(SUM(tx.amount), 0) AS total_amount,
                    COUNT(tx.id) AS tx_count
                FROM tx_data tx
                JOIN categories cat ON cat.id = tx.effective_category_id
                GROUP BY cat.id, cat.name, cat.kind
                ORDER BY total_amount DESC, cat.name ASC
                "#,
            )
            .bind(filter.user_id.as_uuid())
            .bind(filter.from_date)
            .bind(filter.to_date)
            .bind(filter.account_id.map(|id| id.as_uuid()))
            .bind(kind_str)
            .bind(filter.include_pending)
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?
        } else {
            // Separate subcategories and parent categories
            sqlx::query(
                r#"
                SELECT 
                    c.id AS category_id,
                    c.name AS category_name,
                    c.parent_id AS parent_id,
                    p.name AS parent_name,
                    c.kind AS category_kind,
                    COALESCE(SUM(t.amount), 0) AS total_amount,
                    COUNT(t.id) AS tx_count
                FROM transactions t
                JOIN categories c ON c.id = t.category_id
                LEFT JOIN categories p ON p.id = c.parent_id
                WHERE t.user_id = $1
                  AND t.transfer_id IS NULL
                  AND ($2::DATE IS NULL OR t.date >= $2)
                  AND ($3::DATE IS NULL OR t.date <= $3)
                  AND ($4::UUID IS NULL OR t.account_id = $4)
                  AND ($5::TEXT IS NULL OR t.kind = $5)
                  AND ($6::BOOLEAN = TRUE OR t.status = 'paid')
                GROUP BY c.id, c.name, c.parent_id, p.name, c.kind
                ORDER BY total_amount DESC, c.name ASC
                "#,
            )
            .bind(filter.user_id.as_uuid())
            .bind(filter.from_date)
            .bind(filter.to_date)
            .bind(filter.account_id.map(|id| id.as_uuid()))
            .bind(kind_str)
            .bind(filter.include_pending)
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?
        };

        let mut sum_dec = Decimal::ZERO;
        let mut raw_items = Vec::with_capacity(rows.len());

        for row in rows {
            let cat_id: Uuid = row.try_get("category_id").map_err(StorageError::Database)?;
            let cat_name: String = row
                .try_get("category_name")
                .map_err(StorageError::Database)?;
            let parent_id: Option<Uuid> =
                row.try_get("parent_id").map_err(StorageError::Database)?;
            let parent_name: Option<String> =
                row.try_get("parent_name").map_err(StorageError::Database)?;
            let kind_str: String = row
                .try_get("category_kind")
                .map_err(StorageError::Database)?;
            let total_dec: Decimal = row
                .try_get("total_amount")
                .map_err(StorageError::Database)?;
            let tx_count: i64 = row.try_get("tx_count").map_err(StorageError::Database)?;

            let kind = kind_str.parse::<TransactionKind>().map_err(|e| {
                StorageError::Conversion(format!("Tipo de transação inválido '{kind_str}': {e}"))
            })?;

            let total_amount = Money::from_decimal_non_negative(total_dec).map_err(|e| {
                StorageError::Conversion(format!("Valor de categoria inválido: {e}"))
            })?;

            sum_dec += total_dec;
            raw_items.push((
                cat_id,
                cat_name,
                parent_id,
                parent_name,
                kind,
                total_amount,
                total_dec,
                tx_count,
            ));
        }

        let mut items = Vec::with_capacity(raw_items.len());
        for (cat_id, cat_name, parent_id, parent_name, kind, total_amount, total_dec, tx_count) in
            raw_items
        {
            let percentage = if sum_dec > Decimal::ZERO {
                let pct = (total_dec / sum_dec) * Decimal::from(100);
                pct.round_dp(2)
            } else {
                Decimal::ZERO
            };

            items.push(CategoryReportItem {
                category_id: CategoryId::new(cat_id),
                category_name: cat_name,
                parent_id: parent_id.map(CategoryId::new),
                parent_name,
                kind,
                total_amount,
                transaction_count: tx_count,
                percentage,
            });
        }

        let total_money = Money::from_decimal_non_negative(sum_dec).map_err(|e| {
            StorageError::Conversion(format!("Valor total de categorias inválido: {e}"))
        })?;

        Ok(CategoryReportSummary {
            kind: filter.kind,
            total_amount: total_money,
            items,
        })
    }

    pub async fn category_comparison(
        pool: &PgPool,
        filter: CategoryCompareFilter,
    ) -> Result<CategoryComparisonReport, StorageError> {
        let kind_str = filter.kind.map(|k| k.as_str().to_string());

        let rows = sqlx::query(
            r#"
            SELECT 
                TO_CHAR(t.date, 'YYYY-MM') AS month_str,
                c.name AS category_name,
                c.kind AS category_kind,
                COALESCE(SUM(t.amount), 0) AS total_amount
            FROM transactions t
            JOIN categories c ON c.id = t.category_id
            WHERE t.user_id = $1
              AND t.transfer_id IS NULL
              AND t.date >= $2
              AND t.date <= $3
              AND ($4::UUID IS NULL OR t.account_id = $4)
              AND ($5::TEXT IS NULL OR t.kind = $5)
              AND ($6::BOOLEAN = TRUE OR t.status = 'paid')
            GROUP BY month_str, c.name, c.kind
            ORDER BY c.name ASC, month_str ASC
            "#,
        )
        .bind(filter.user_id.as_uuid())
        .bind(filter.from_date)
        .bind(filter.to_date)
        .bind(filter.account_id.map(|id| id.as_uuid()))
        .bind(kind_str)
        .bind(filter.include_pending)
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        // Map: (CategoryName, Kind) -> Map(Month -> Decimal)
        let mut category_map: BTreeMap<(String, TransactionKind), BTreeMap<String, Decimal>> =
            BTreeMap::new();

        for row in rows {
            let month_str: String = row.try_get("month_str").map_err(StorageError::Database)?;
            let cat_name: String = row
                .try_get("category_name")
                .map_err(StorageError::Database)?;
            let kind_str: String = row
                .try_get("category_kind")
                .map_err(StorageError::Database)?;
            let total_dec: Decimal = row
                .try_get("total_amount")
                .map_err(StorageError::Database)?;

            let kind = kind_str.parse::<TransactionKind>().map_err(|e| {
                StorageError::Conversion(format!("Tipo de transação inválido '{kind_str}': {e}"))
            })?;

            category_map
                .entry((cat_name, kind))
                .or_default()
                .insert(month_str, total_dec);
        }

        let mut rows_out = Vec::new();
        for ((cat_name, kind), month_values) in category_map {
            let mut monthly_amounts = Vec::new();
            let mut amounts_dec = Vec::new();

            for m in &filter.months {
                let dec = month_values.get(m).copied().unwrap_or(Decimal::ZERO);
                let money = Money::from_decimal_non_negative(dec).map_err(|e| {
                    StorageError::Conversion(format!("Valor inválido para quantia: {e}"))
                })?;
                monthly_amounts.push((m.clone(), money));
                amounts_dec.push(dec);
            }

            // Calculate diff between the last month and previous month (or first and last if 2)
            let (absolute_diff, percent_diff) = if amounts_dec.len() >= 2 {
                let prev = amounts_dec[amounts_dec.len() - 2];
                let curr = amounts_dec[amounts_dec.len() - 1];
                let abs_diff = curr - prev;
                let pct_diff = if prev > Decimal::ZERO {
                    let pct = (abs_diff / prev) * Decimal::from(100);
                    Some(pct.round_dp(2))
                } else {
                    None // Base zero: n/d
                };
                (abs_diff, pct_diff)
            } else {
                (Decimal::ZERO, None)
            };

            rows_out.push(CategoryComparisonRow {
                category_name: cat_name,
                kind,
                monthly_amounts,
                absolute_diff,
                percent_diff,
            });
        }

        Ok(CategoryComparisonReport {
            months: filter.months,
            rows: rows_out,
        })
    }
}
