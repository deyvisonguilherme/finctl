use crate::errors::DomainError;
use crate::types::{AccountId, CardInvoiceId, InvoiceStatus, UserId};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInvoice {
    pub id: CardInvoiceId,
    pub user_id: UserId,
    pub account_id: AccountId,
    pub month: String, // "YYYY-MM"
    pub closing_date: NaiveDate,
    pub due_date: NaiveDate,
    pub status: InvoiceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CardInvoice {
    pub fn new(
        user_id: UserId,
        account_id: AccountId,
        month: String,
        closing_date: NaiveDate,
        due_date: NaiveDate,
    ) -> Result<Self, DomainError> {
        Self::new_with_status(
            user_id,
            account_id,
            month,
            closing_date,
            due_date,
            InvoiceStatus::Open,
        )
    }

    pub fn new_with_status(
        user_id: UserId,
        account_id: AccountId,
        month: String,
        closing_date: NaiveDate,
        due_date: NaiveDate,
        status: InvoiceStatus,
    ) -> Result<Self, DomainError> {
        let trimmed_month = month.trim().to_string();
        if trimmed_month.len() != 7 || !trimmed_month.contains('-') {
            return Err(DomainError::Validation(format!(
                "Mês de referência inválido '{month}'. Use o formato AAAA-MM."
            )));
        }

        let now = Utc::now();
        Ok(Self {
            id: CardInvoiceId::generate(),
            user_id,
            account_id,
            month: trimmed_month,
            closing_date,
            due_date,
            status,
            created_at: now,
            updated_at: now,
        })
    }
}

/// Clampa o dia para o último dia válido do mês caso o mês seja mais curto (decisão D-05).
pub fn clamp_day_to_month(year: i32, month: u32, day: u8) -> NaiveDate {
    let last_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => 30,
    };

    let target_day = (day as u32).min(last_day);
    NaiveDate::from_ymd_opt(year, month, target_day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month, 1).unwrap())
}

/// Calcula datas de fechamento e vencimento para um mês de referência AAAA-MM
pub fn calculate_invoice_dates_for_month(
    closing_day: u8,
    due_day: u8,
    year: i32,
    month: u32,
) -> (NaiveDate, NaiveDate) {
    let closing_date = clamp_day_to_month(year, month, closing_day);

    let due_date = if due_day > closing_day {
        clamp_day_to_month(year, month, due_day)
    } else {
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        clamp_day_to_month(next_year, next_month, due_day)
    };

    (closing_date, due_date)
}

/// Determina o mês de referência e as datas da fatura para uma compra realizada em `tx_date`.
/// Se a compra ocorreu no dia do fechamento ou antes, cai na fatura do mês atual.
/// Se ocorreu após o dia de fechamento, cai na fatura do mês seguinte.
pub fn calculate_invoice_dates_for_transaction(
    closing_day: u8,
    due_day: u8,
    tx_date: NaiveDate,
) -> (String, NaiveDate, NaiveDate) {
    let year = tx_date.year();
    let month = tx_date.month();

    let (closing_cur, due_cur) =
        calculate_invoice_dates_for_month(closing_day, due_day, year, month);

    if tx_date <= closing_cur {
        (format!("{year:04}-{month:02}"), closing_cur, due_cur)
    } else {
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let (closing_next, due_next) =
            calculate_invoice_dates_for_month(closing_day, due_day, next_year, next_month);
        (
            format!("{next_year:04}-{next_month:02}"),
            closing_next,
            due_next,
        )
    }
}

/// Calcula o próximo mês de referência ("AAAA-MM")
pub fn next_reference_month(month: &str) -> Result<String, DomainError> {
    let parts: Vec<&str> = month.split('-').collect();
    if parts.len() != 2 {
        return Err(DomainError::Validation(format!(
            "Mês inválido '{month}'. Use AAAA-MM."
        )));
    }

    let year: i32 = parts[0]
        .parse()
        .map_err(|_| DomainError::Validation(format!("Ano inválido em '{month}'.")))?;
    let m: u32 = parts[1]
        .parse()
        .map_err(|_| DomainError::Validation(format!("Mês numérico inválido em '{month}'.")))?;

    let (ny, nm) = if m == 12 {
        (year + 1, 1)
    } else {
        (year, m + 1)
    };
    Ok(format!("{ny:04}-{nm:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closing_day_purchase_attribution() {
        // closing_day: 20, due_day: 27
        let tx_on_closing = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let (month_cur, closing_cur, due_cur) =
            calculate_invoice_dates_for_transaction(20, 27, tx_on_closing);
        assert_eq!(month_cur, "2026-05");
        assert_eq!(closing_cur, NaiveDate::from_ymd_opt(2026, 5, 20).unwrap());
        assert_eq!(due_cur, NaiveDate::from_ymd_opt(2026, 5, 27).unwrap());

        // Day after closing: 2026-05-21 falls in next invoice 2026-06
        let tx_after_closing = NaiveDate::from_ymd_opt(2026, 5, 21).unwrap();
        let (month_next, closing_next, due_next) =
            calculate_invoice_dates_for_transaction(20, 27, tx_after_closing);
        assert_eq!(month_next, "2026-06");
        assert_eq!(closing_next, NaiveDate::from_ymd_opt(2026, 6, 20).unwrap());
        assert_eq!(due_next, NaiveDate::from_ymd_opt(2026, 6, 27).unwrap());
    }

    #[test]
    fn test_due_day_next_month_when_smaller_than_closing_day() {
        // closing_day: 25, due_day: 5 -> due date is in next month
        let (closing, due) = calculate_invoice_dates_for_month(25, 5, 2026, 5);
        assert_eq!(closing, NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(due, NaiveDate::from_ymd_opt(2026, 6, 5).unwrap());
    }

    #[test]
    fn test_d05_short_months() {
        // closing_day 31 in Feb 2026 (non leap) -> Feb 28
        let (closing, _) = calculate_invoice_dates_for_month(31, 10, 2026, 2);
        assert_eq!(closing, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn test_next_reference_month() {
        assert_eq!(next_reference_month("2026-05").unwrap(), "2026-06");
        assert_eq!(next_reference_month("2026-12").unwrap(), "2027-01");
    }
}
