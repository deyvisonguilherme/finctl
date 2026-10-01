use crate::errors::AppError;
use chrono::{Datelike, Local, NaiveDate};
use domain::{CategoryReportSummary, MonthlyReportItem, TransactionKind, UserId};
use sqlx::PgPool;
use storage::{AccountRepository, CategoryReportFilter, MonthlyReportFilter, ReportRepository};

#[derive(Debug, Clone, Default)]
pub struct MonthlyReportInput {
    pub user_id: UserId,
    pub month: Option<String>,
    pub year: Option<i32>,
    pub account_query: Option<String>,
    pub include_pending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CategoryReportInput {
    pub user_id: UserId,
    pub month: Option<String>,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub account_query: Option<String>,
    pub kind: Option<TransactionKind>,
    pub depth: u32,
    pub include_pending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CompareReportInput {
    pub user_id: UserId,
    pub months: Option<Vec<String>>,
    pub last_n: Option<u32>,
    pub account_query: Option<String>,
    pub include_pending: bool,
}

pub struct ReportService<'a> {
    pool: &'a PgPool,
}

impl<'a> ReportService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn monthly_report(
        &self,
        input: MonthlyReportInput,
    ) -> Result<Vec<MonthlyReportItem>, AppError> {
        let account_id = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc.id)
        } else {
            None
        };

        let (from_date, to_date) = if let Some(ref m) = input.month {
            let (start, end) = parse_month_bounds(m)?;
            (Some(start), Some(end))
        } else if let Some(y) = input.year {
            let start = NaiveDate::from_ymd_opt(y, 1, 1)
                .ok_or_else(|| AppError::Validation(format!("Ano inválido '{y}'")))?;
            let end = NaiveDate::from_ymd_opt(y, 12, 31)
                .ok_or_else(|| AppError::Validation(format!("Ano inválido '{y}'")))?;
            (Some(start), Some(end))
        } else {
            // Padrão: mês atual
            let today = Local::now().date_naive();
            let current_month_str = format!("{:04}-{:02}", today.year(), today.month());
            let (start, end) = parse_month_bounds(&current_month_str)?;
            (Some(start), Some(end))
        };

        let filter = MonthlyReportFilter {
            user_id: input.user_id,
            from_date,
            to_date,
            account_id,
            include_pending: input.include_pending,
        };

        let items = ReportRepository::monthly_summary(self.pool, filter).await?;
        Ok(items)
    }

    pub async fn category_report(
        &self,
        input: CategoryReportInput,
    ) -> Result<CategoryReportSummary, AppError> {
        let account_id = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc.id)
        } else {
            None
        };

        let (from_date, to_date) = if let Some(ref m) = input.month {
            let (start, end) = parse_month_bounds(m)?;
            (Some(start), Some(end))
        } else if input.from_date.is_some() || input.to_date.is_some() {
            (input.from_date, input.to_date)
        } else {
            // Padrão: mês atual
            let today = Local::now().date_naive();
            let current_month_str = format!("{:04}-{:02}", today.year(), today.month());
            let (start, end) = parse_month_bounds(&current_month_str)?;
            (Some(start), Some(end))
        };

        let depth = if input.depth == 1 { 1 } else { 2 };

        let filter = CategoryReportFilter {
            user_id: input.user_id,
            from_date,
            to_date,
            account_id,
            kind: input.kind,
            depth,
            include_pending: input.include_pending,
        };

        let summary = ReportRepository::category_summary(self.pool, filter).await?;
        Ok(summary)
    }

    pub async fn compare_report(
        &self,
        input: CompareReportInput,
    ) -> Result<Vec<MonthlyReportItem>, AppError> {
        let account_id = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc.id)
        } else {
            None
        };

        let (from_date, to_date) = if let Some(ref months) = input.months {
            if months.is_empty() {
                return Err(AppError::Validation(
                    "Pelo menos um mês deve ser especificado para comparação.".to_string(),
                ));
            }
            let mut min_date = NaiveDate::MAX;
            let mut max_date = NaiveDate::MIN;

            for m in months {
                let (start, end) = parse_month_bounds(m)?;
                if start < min_date {
                    min_date = start;
                }
                if end > max_date {
                    max_date = end;
                }
            }

            (Some(min_date), Some(max_date))
        } else {
            let last_n = input.last_n.unwrap_or(3);
            if last_n == 0 {
                return Err(AppError::Validation(
                    "O número de meses deve ser maior que 0.".to_string(),
                ));
            }

            let today = Local::now().date_naive();
            let current_year = today.year();
            let current_month = today.month();

            // Calculate start month: (last_n - 1) months ago
            let total_months = current_year * 12 + (current_month as i32 - 1);
            let start_total_months = total_months - (last_n as i32 - 1);
            let start_year = start_total_months / 12;
            let start_month = (start_total_months % 12 + 1) as u32;

            let start_date = NaiveDate::from_ymd_opt(start_year, start_month, 1)
                .ok_or_else(|| AppError::Validation("Data de início inválida.".to_string()))?;

            let next_month = if current_month == 12 {
                NaiveDate::from_ymd_opt(current_year + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(current_year, current_month + 1, 1).unwrap()
            };
            let end_date = next_month.pred_opt().unwrap_or(today);

            (Some(start_date), Some(end_date))
        };

        let filter = MonthlyReportFilter {
            user_id: input.user_id,
            from_date,
            to_date,
            account_id,
            include_pending: input.include_pending,
        };

        let mut items = ReportRepository::monthly_summary(self.pool, filter).await?;

        // If specific months were requested, filter to only those months
        if let Some(ref months) = input.months {
            items.retain(|item| months.contains(&item.month));
        }

        Ok(items)
    }
}

pub fn parse_month_bounds(month_str: &str) -> Result<(NaiveDate, NaiveDate), AppError> {
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
