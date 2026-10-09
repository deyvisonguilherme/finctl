use crate::errors::DomainError;
use crate::types::AccountId;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ForecastGranularity {
    Week,
    #[default]
    Month,
}

impl fmt::Display for ForecastGranularity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForecastGranularity::Week => write!(f, "week"),
            ForecastGranularity::Month => write!(f, "month"),
        }
    }
}

impl FromStr for ForecastGranularity {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "week" | "semana" => Ok(ForecastGranularity::Week),
            "month" | "mes" | "mês" => Ok(ForecastGranularity::Month),
            other => Err(DomainError::Validation(format!(
                "Granularidade inválida '{other}'. Use 'week' ou 'month'."
            ))),
        }
    }
}

/// Um período agregado na projeção de fluxo de caixa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForecastPeriod {
    pub period_label: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub opening_balance: Decimal,
    pub total_income: Decimal,
    pub total_expense: Decimal,
    pub net_change: Decimal,
    pub closing_balance: Decimal,
    pub is_negative: bool,
}

/// Projeção consolidada de fluxo de caixa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashflowForecast {
    pub as_of_date: NaiveDate,
    pub granularity: ForecastGranularity,
    pub months: u32,
    pub account_id: Option<AccountId>,
    pub account_name: Option<String>,
    pub initial_balance: Decimal,
    pub periods: Vec<ForecastPeriod>,
    pub first_negative_period: Option<String>,
    pub lowest_projected_balance: Decimal,
}

impl CashflowForecast {
    pub fn new(
        as_of_date: NaiveDate,
        granularity: ForecastGranularity,
        months: u32,
        account_id: Option<AccountId>,
        account_name: Option<String>,
        initial_balance: Decimal,
        periods: Vec<ForecastPeriod>,
    ) -> Self {
        let first_negative_period = periods
            .iter()
            .find(|p| p.is_negative)
            .map(|p| p.period_label.clone());

        let lowest_projected_balance = periods
            .iter()
            .map(|p| p.closing_balance)
            .min()
            .unwrap_or(initial_balance);

        Self {
            as_of_date,
            granularity,
            months,
            account_id,
            account_name,
            initial_balance,
            periods,
            first_negative_period,
            lowest_projected_balance,
        }
    }
}

/// Helper para gerar intervalos de períodos para a projeção.
pub fn generate_forecast_intervals(
    start_date: NaiveDate,
    end_date: NaiveDate,
    granularity: ForecastGranularity,
) -> Vec<(String, NaiveDate, NaiveDate)> {
    let mut intervals = Vec::new();

    match granularity {
        ForecastGranularity::Month => {
            let mut current = start_date;
            while current <= end_date {
                let year = current.year();
                let month = current.month();
                let label = format!("{year:04}-{month:02}");

                let period_start = current;
                let last_day = last_day_of_month(year, month);
                let month_end = NaiveDate::from_ymd_opt(year, month, last_day).unwrap();
                let period_end = month_end.min(end_date);

                intervals.push((label, period_start, period_end));

                // Avança para o primeiro dia do próximo mês
                current = if month == 12 {
                    NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
                };
            }
        }
        ForecastGranularity::Week => {
            let mut current = start_date;
            while current <= end_date {
                let iso_week = current.iso_week();
                let label = format!("{:04}-W{:02}", iso_week.year(), iso_week.week());

                let period_start = current;
                // Domingo da semana atual
                let days_until_sunday = 7 - current.weekday().number_from_monday();
                let sunday = current + chrono::Duration::days(days_until_sunday as i64);
                let period_end = sunday.min(end_date);

                intervals.push((label, period_start, period_end));

                // Avança para a próxima segunda-feira
                current = sunday + chrono::Duration::days(1);
            }
        }
    }

    intervals
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    next_month_first.pred_opt().unwrap().day()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_generate_month_intervals() {
        let start = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let intervals = generate_forecast_intervals(start, end, ForecastGranularity::Month);

        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0].0, "2026-10");
        assert_eq!(intervals[0].1, start);
        assert_eq!(
            intervals[0].2,
            NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()
        );

        assert_eq!(intervals[1].0, "2026-11");
        assert_eq!(
            intervals[1].1,
            NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
        );
        assert_eq!(
            intervals[1].2,
            NaiveDate::from_ymd_opt(2026, 11, 30).unwrap()
        );

        assert_eq!(intervals[2].0, "2026-12");
    }

    #[test]
    fn test_generate_week_intervals() {
        // Quinta-feira, 2026-10-15
        let start = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        // Domingo da semana seguinte, 2026-10-25
        let end = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let intervals = generate_forecast_intervals(start, end, ForecastGranularity::Week);

        assert_eq!(intervals.len(), 2);
        // Primeiro intervalo vai de quinta (15) até domingo (18)
        assert_eq!(intervals[0].1, start);
        assert_eq!(
            intervals[0].2,
            NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()
        );
        // Segundo intervalo vai de segunda (19) até domingo (25)
        assert_eq!(
            intervals[1].1,
            NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
        );
        assert_eq!(intervals[1].2, end);
    }

    #[test]
    fn test_cashflow_forecast_detects_first_negative_period() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let p1 = ForecastPeriod {
            period_label: "2026-10".to_string(),
            start_date: today,
            end_date: NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
            opening_balance: dec!(1000.00),
            total_income: dec!(500.00),
            total_expense: dec!(200.00),
            net_change: dec!(300.00),
            closing_balance: dec!(1300.00),
            is_negative: false,
        };
        let p2 = ForecastPeriod {
            period_label: "2026-11".to_string(),
            start_date: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
            opening_balance: dec!(1300.00),
            total_income: dec!(200.00),
            total_expense: dec!(2000.00),
            net_change: dec!(-1800.00),
            closing_balance: dec!(-500.00),
            is_negative: true,
        };

        let forecast = CashflowForecast::new(
            today,
            ForecastGranularity::Month,
            2,
            None,
            None,
            dec!(1000.00),
            vec![p1, p2],
        );

        assert_eq!(forecast.first_negative_period, Some("2026-11".to_string()));
        assert_eq!(forecast.lowest_projected_balance, dec!(-500.00));
    }
}
