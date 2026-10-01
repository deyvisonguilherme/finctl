use crate::errors::AppError;
use chrono::NaiveDate;
use domain::{Budget, BudgetIndicator, CategoryId, Money, TransactionKind, UserId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::{BudgetDetails, BudgetRepository, CategoryRepository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBudgetStatus {
    pub category_id: CategoryId,
    pub category_name: String,
    pub budget_amount: Money,
    pub consumed_amount: Money,
    pub remaining_amount: Decimal,
    pub percentage: Decimal,
    pub indicator: BudgetIndicator,
    pub is_monthly_exception: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub category_name: String,
    pub budget_amount: Money,
    pub consumed_amount: Money,
    pub percentage: Decimal,
    pub indicator: BudgetIndicator,
}

pub struct BudgetService<'a> {
    pool: &'a PgPool,
}

impl<'a> BudgetService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn set_budget(
        &self,
        user_id: UserId,
        category_query: String,
        amount: Money,
        month: Option<String>,
    ) -> Result<Budget, AppError> {
        let category = CategoryRepository::find_by_id_or_name(self.pool, user_id, &category_query)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Categoria '{category_query}' não encontrada."))
            })?;

        if category.kind != TransactionKind::Expense {
            return Err(AppError::Validation(format!(
                "Orçamentos só podem ser definidos para categorias de despesa. A categoria '{}' é do tipo '{}'.",
                category.name,
                category.kind.display_pt_br()
            )));
        }

        let budget = Budget::new(user_id, category.id, amount, month)?;
        BudgetRepository::upsert(self.pool, &budget).await?;
        Ok(budget)
    }

    pub async fn list_budgets(
        &self,
        user_id: UserId,
        month: Option<String>,
    ) -> Result<Vec<BudgetDetails>, AppError> {
        let budgets = BudgetRepository::list_by_user(self.pool, user_id, month.as_deref()).await?;
        Ok(budgets)
    }

    pub async fn get_budget_status(
        &self,
        user_id: UserId,
        month: String,
    ) -> Result<Vec<CategoryBudgetStatus>, AppError> {
        let (start_date, end_date) = parse_month_bounds(&month)?;
        let active_budgets =
            BudgetRepository::find_active_budgets_for_month(self.pool, user_id, &month).await?;
        let all_categories = CategoryRepository::list_by_user(self.pool, user_id).await?;

        let mut statuses = Vec::with_capacity(active_budgets.len());

        for b in active_budgets {
            // Include category itself and all subcategories
            let mut cat_ids = vec![b.category_id.as_uuid()];
            for cat in &all_categories {
                if let Some(parent_id) = cat.parent_id {
                    if parent_id == b.category_id {
                        cat_ids.push(cat.id.as_uuid());
                    }
                }
            }

            let consumed_dec: Decimal = sqlx::query_scalar(
                r#"
                SELECT COALESCE(SUM(amount), 0.00)
                FROM transactions
                WHERE user_id = $1
                  AND category_id = ANY($2)
                  AND kind = 'expense'
                  AND status = 'paid'
                  AND transfer_id IS NULL
                  AND date >= $3 AND date <= $4
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(&cat_ids)
            .bind(start_date)
            .bind(end_date)
            .fetch_one(self.pool)
            .await
            .map_err(storage::StorageError::Database)?;

            let consumed_amount =
                Money::from_decimal_non_negative(consumed_dec).unwrap_or(Money::ZERO);
            let budget_dec = b.amount.as_decimal();
            let remaining = budget_dec - consumed_dec;

            let percentage = if budget_dec > Decimal::ZERO {
                (consumed_dec / budget_dec) * Decimal::from(100)
            } else {
                Decimal::ZERO
            };

            let indicator = BudgetIndicator::from_percentage(percentage);

            statuses.push(CategoryBudgetStatus {
                category_id: b.category_id,
                category_name: b.category_name,
                budget_amount: b.amount,
                consumed_amount,
                remaining_amount: remaining,
                percentage,
                indicator,
                is_monthly_exception: b.month.is_some(),
            });
        }

        // Sort: highest percentage first, then alphabetically
        statuses.sort_by(|a, b| {
            b.percentage
                .cmp(&a.percentage)
                .then_with(|| a.category_name.cmp(&b.category_name))
        });

        Ok(statuses)
    }

    pub async fn check_budget_after_expense(
        &self,
        user_id: UserId,
        category_id: CategoryId,
        date: NaiveDate,
    ) -> Result<Option<BudgetAlert>, AppError> {
        let month = date.format("%Y-%m").to_string();
        let all_categories = CategoryRepository::list_by_user(self.pool, user_id).await?;

        // Find the category and possible parent
        let category = all_categories.iter().find(|c| c.id == category_id);
        let target_category = match category {
            Some(c) => c,
            None => return Ok(None),
        };

        // Check if there is budget for this category or for its parent
        let mut budget_cat_id = target_category.id;
        let mut maybe_budget = BudgetRepository::find_for_category_and_month(
            self.pool,
            user_id,
            budget_cat_id,
            &month,
        )
        .await?;

        if maybe_budget.is_none() {
            if let Some(parent_id) = target_category.parent_id {
                budget_cat_id = parent_id;
                maybe_budget = BudgetRepository::find_for_category_and_month(
                    self.pool,
                    user_id,
                    budget_cat_id,
                    &month,
                )
                .await?;
            }
        }

        let budget_info = match maybe_budget {
            Some(b) => b,
            None => return Ok(None),
        };

        // Calculate consumed amount for this budget
        let (start_date, end_date) = parse_month_bounds(&month)?;
        let mut cat_ids = vec![budget_cat_id.as_uuid()];
        for cat in &all_categories {
            if let Some(parent_id) = cat.parent_id {
                if parent_id == budget_cat_id {
                    cat_ids.push(cat.id.as_uuid());
                }
            }
        }

        let consumed_dec: Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(amount), 0.00)
            FROM transactions
            WHERE user_id = $1
              AND category_id = ANY($2)
              AND kind = 'expense'
              AND status = 'paid'
              AND transfer_id IS NULL
              AND date >= $3 AND date <= $4
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(&cat_ids)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(self.pool)
        .await
        .map_err(storage::StorageError::Database)?;

        let budget_dec = budget_info.amount.as_decimal();
        let percentage = if budget_dec > Decimal::ZERO {
            (consumed_dec / budget_dec) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        let indicator = BudgetIndicator::from_percentage(percentage);

        if indicator == BudgetIndicator::Warning || indicator == BudgetIndicator::Exceeded {
            let consumed_amount =
                Money::from_decimal_non_negative(consumed_dec).unwrap_or(Money::ZERO);
            Ok(Some(BudgetAlert {
                category_name: budget_info.category_name,
                budget_amount: budget_info.amount,
                consumed_amount,
                percentage,
                indicator,
            }))
        } else {
            Ok(None)
        }
    }
}

fn parse_month_bounds(month_str: &str) -> Result<(NaiveDate, NaiveDate), AppError> {
    let parts: Vec<&str> = month_str.split('-').collect();
    if parts.len() != 2 {
        return Err(AppError::Validation(format!(
            "Formato de mês inválido '{month_str}'. Use o formato AAAA-MM (ex: 2026-10)."
        )));
    }
    let year: i32 = parts[0]
        .parse()
        .map_err(|_| AppError::Validation(format!("Ano inválido '{}'", parts[0])))?;
    let month: u32 = parts[1]
        .parse()
        .map_err(|_| AppError::Validation(format!("Mês inválido '{}'", parts[1])))?;
    if !(1..=12).contains(&month) {
        return Err(AppError::Validation(format!(
            "Mês inválido '{month}'. Deve ser entre 01 e 12."
        )));
    }

    let start = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::Validation("Data de início de mês inválida.".to_string()))?;

    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    let end = next_month.pred_opt().unwrap_or(start);

    Ok((start, end))
}
