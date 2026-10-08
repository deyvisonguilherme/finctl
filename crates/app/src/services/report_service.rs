use crate::errors::AppError;
use chrono::{Datelike, Local, NaiveDate};
use domain::{
    CategoryComparisonReport, CategoryReportSummary, Money, MonthlyReportItem, TransactionKind,
    UserId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::{
    AccountRepository, CategoryCompareFilter, CategoryReportFilter, MonthlyReportFilter,
    ReportRepository,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportsScreenData {
    pub reference_month: String,
    pub include_pending: bool,
    pub category_report: CategoryReportSummary,
    pub monthly_history: Vec<MonthlyReportItem>,
    pub comparison_categories: CategoryComparisonReport,
    pub comparison_totals: Vec<MonthlyReportItem>,
}

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
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CompareReportInput {
    pub user_id: UserId,
    pub months: Option<Vec<String>>,
    pub last_n: Option<u32>,
    pub account_query: Option<String>,
    pub include_pending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CompareCategoriesInput {
    pub user_id: UserId,
    pub months: Option<Vec<String>>,
    pub last_n: Option<u32>,
    pub kind: Option<TransactionKind>,
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
            tag: input.tag,
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

        if let Some(ref months) = input.months {
            items.retain(|item| months.contains(&item.month));
        }

        Ok(items)
    }

    pub async fn compare_categories(
        &self,
        input: CompareCategoriesInput,
    ) -> Result<CategoryComparisonReport, AppError> {
        let account_id = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc.id)
        } else {
            None
        };

        let (months_list, from_date, to_date) = if let Some(mut months) = input.months {
            if months.is_empty() {
                return Err(AppError::Validation(
                    "Pelo menos um mês deve ser especificado para comparação.".to_string(),
                ));
            }
            months.sort();
            months.dedup();

            let mut min_date = NaiveDate::MAX;
            let mut max_date = NaiveDate::MIN;

            for m in &months {
                let (start, end) = parse_month_bounds(m)?;
                if start < min_date {
                    min_date = start;
                }
                if end > max_date {
                    max_date = end;
                }
            }

            (months, min_date, max_date)
        } else {
            let last_n = input.last_n.unwrap_or(2);
            if last_n == 0 {
                return Err(AppError::Validation(
                    "O número de meses deve ser maior que 0.".to_string(),
                ));
            }

            let today = Local::now().date_naive();
            let current_year = today.year();
            let current_month = today.month();

            let total_months = current_year * 12 + (current_month as i32 - 1);
            let start_total_months = total_months - (last_n as i32 - 1);

            let mut months = Vec::with_capacity(last_n as usize);
            let mut min_date = NaiveDate::MAX;
            let mut max_date = NaiveDate::MIN;

            for i in 0..last_n {
                let m_total = start_total_months + i as i32;
                let y = m_total / 12;
                let m = (m_total % 12 + 1) as u32;
                let m_str = format!("{y:04}-{m:02}");
                let (start, end) = parse_month_bounds(&m_str)?;
                if start < min_date {
                    min_date = start;
                }
                if end > max_date {
                    max_date = end;
                }
                months.push(m_str);
            }

            (months, min_date, max_date)
        };

        let filter = CategoryCompareFilter {
            user_id: input.user_id,
            months: months_list,
            from_date,
            to_date,
            account_id,
            kind: input.kind,
            include_pending: input.include_pending,
        };

        let report = ReportRepository::category_comparison(self.pool, filter).await?;
        Ok(report)
    }

    pub async fn get_reports_screen_data(
        &self,
        user_id: UserId,
        reference_month: &str,
        include_pending: bool,
    ) -> Result<ReportsScreenData, AppError> {
        let (ref_start, ref_end) = parse_month_bounds(reference_month)?;
        let ref_year = ref_start.year();
        let ref_month = ref_start.month();

        // 1. Gastos por categoria para o mês de referência
        let category_report = self
            .category_report(CategoryReportInput {
                user_id,
                month: Some(reference_month.to_string()),
                from_date: None,
                to_date: None,
                account_query: None,
                kind: Some(TransactionKind::Expense),
                depth: 1,
                include_pending,
                tag: None,
            })
            .await?;

        // 2. Histórico dos últimos 6 meses para Sparklines e tabela de evolução
        let mut last_6_months = Vec::new();
        let total_m = ref_year * 12 + (ref_month as i32 - 1);
        for offset in (0..6).rev() {
            let m_val = total_m - offset;
            let y = m_val / 12;
            let m = (m_val % 12 + 1) as u32;
            last_6_months.push(format!("{:04}-{:02}", y, m));
        }

        let earliest_month = &last_6_months[0];
        let (earliest_start, _) = parse_month_bounds(earliest_month)?;

        let history_filter = MonthlyReportFilter {
            user_id,
            from_date: Some(earliest_start),
            to_date: Some(ref_end),
            account_id: None,
            include_pending,
        };
        let queried_history = ReportRepository::monthly_summary(self.pool, history_filter).await?;

        let monthly_history: Vec<MonthlyReportItem> = last_6_months
            .iter()
            .map(|m_str| {
                if let Some(item) = queried_history.iter().find(|h| &h.month == m_str) {
                    item.clone()
                } else {
                    MonthlyReportItem {
                        month: m_str.clone(),
                        total_income: Money::ZERO,
                        total_expense: Money::ZERO,
                        net_balance: Decimal::ZERO,
                        savings_rate: Decimal::ZERO,
                    }
                }
            })
            .collect();

        // 3. Mês anterior para Comparativo
        let prev_month = if ref_month == 1 {
            format!("{:04}-12", ref_year - 1)
        } else {
            format!("{:04}-{:02}", ref_year, ref_month - 1)
        };

        // 4. Comparativo por categorias (Mês Anterior vs Mês de Referência)
        let comparison_categories = self
            .compare_categories(CompareCategoriesInput {
                user_id,
                months: Some(vec![prev_month.clone(), reference_month.to_string()]),
                last_n: None,
                kind: Some(TransactionKind::Expense),
                account_query: None,
                include_pending,
            })
            .await?;

        // 5. Comparativo de totais (Receitas, Despesas, Saldo) entre os dois meses
        let comparison_totals = self
            .compare_report(CompareReportInput {
                user_id,
                months: Some(vec![prev_month, reference_month.to_string()]),
                last_n: None,
                account_query: None,
                include_pending,
            })
            .await?;

        Ok(ReportsScreenData {
            reference_month: reference_month.to_string(),
            include_pending,
            category_report,
            monthly_history,
            comparison_categories,
            comparison_totals,
        })
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
