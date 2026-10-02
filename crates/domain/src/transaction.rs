use crate::errors::DomainError;
use crate::money::Money;
use crate::types::{
    AccountId, CategoryId, TransactionId, TransactionKind, TransactionStatus, UserId,
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub user_id: UserId,
    pub account_id: AccountId,
    pub category_id: CategoryId,
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
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Transaction {
    pub fn new(
        user_id: UserId,
        account_id: AccountId,
        category_id: CategoryId,
        kind: TransactionKind,
        amount: Money,
        date: NaiveDate,
        description: String,
    ) -> Result<Self, DomainError> {
        Self::new_full(
            user_id,
            account_id,
            category_id,
            kind,
            amount,
            date,
            description,
            TransactionStatus::Paid,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_full(
        user_id: UserId,
        account_id: AccountId,
        category_id: CategoryId,
        kind: TransactionKind,
        amount: Money,
        date: NaiveDate,
        description: String,
        status: TransactionStatus,
        transfer_id: Option<Uuid>,
        installment_group_id: Option<Uuid>,
        installment_number: Option<u32>,
        installment_total: Option<u32>,
        recurring_rule_id: Option<Uuid>,
        import_hash: Option<String>,
        reconciled_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        let trimmed_desc = description.trim().to_string();
        if trimmed_desc.len() > 255 {
            return Err(DomainError::Validation(
                "A descrição não pode ter mais de 255 caracteres.".to_string(),
            ));
        }

        if let (Some(num), Some(tot)) = (installment_number, installment_total) {
            if num == 0 || tot == 0 {
                return Err(DomainError::Validation(
                    "Número e total de parcelas devem ser maiores que zero.".to_string(),
                ));
            }
            if num > tot {
                return Err(DomainError::Validation(
                    "O número da parcela não pode ser maior que o total de parcelas.".to_string(),
                ));
            }
        }

        let now = Utc::now();
        Ok(Self {
            id: TransactionId::generate(),
            user_id,
            account_id,
            category_id,
            kind,
            amount,
            date,
            description: trimmed_desc,
            status,
            transfer_id,
            installment_group_id,
            installment_number,
            installment_total,
            recurring_rule_id,
            import_hash,
            reconciled_at,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }

    pub fn mark_as_paid(&mut self, payment_date: Option<NaiveDate>) -> Result<(), DomainError> {
        if self.status == TransactionStatus::Paid {
            return Err(DomainError::Validation(
                "Lançamento já está marcado como realizado (pago).".to_string(),
            ));
        }
        self.status = TransactionStatus::Paid;
        if let Some(date) = payment_date {
            self.date = date;
        }
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn soft_delete(&mut self) {
        self.deleted_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn restore(&mut self) {
        self.deleted_at = None;
        self.updated_at = Utc::now();
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

pub fn split_installments(
    total_amount: Money,
    installments: u32,
) -> Result<Vec<Money>, DomainError> {
    if installments < 2 {
        return Err(DomainError::Validation(
            "O número de parcelas deve ser no mínimo 2.".to_string(),
        ));
    }
    let total_dec = total_amount.as_decimal();
    let n = rust_decimal::Decimal::from(installments);
    let base_dec =
        (total_dec / n).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero);
    let remainder = total_dec - (base_dec * n);

    let first_dec = base_dec + remainder;
    let first = Money::new(first_dec)?;
    let base = Money::new(base_dec)?;

    let mut result = Vec::with_capacity(installments as usize);
    result.push(first);
    for _ in 1..installments {
        result.push(base);
    }
    Ok(result)
}

pub fn calculate_installment_dates(start_date: NaiveDate, installments: u32) -> Vec<NaiveDate> {
    let mut dates = Vec::with_capacity(installments as usize);
    let start_dom = start_date.day();
    let start_year = start_date.year();
    let start_month0 = start_date.month0() as i32;

    for i in 0..installments {
        let total_month = start_month0 + i as i32;
        let year = start_year + (total_month / 12);
        let month = (total_month % 12 + 1) as u32;
        let dim = crate::recurring::days_in_month(year, month);
        let day = start_dom.min(dim);
        if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
            dates.push(d);
        }
    }
    dates
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_create_transaction_success() {
        let user_id = UserId::generate();
        let account_id = AccountId::generate();
        let category_id = CategoryId::generate();
        let amount = Money::new(dec!(89.90)).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();

        let tx = Transaction::new(
            user_id,
            account_id,
            category_id,
            TransactionKind::Expense,
            amount,
            date,
            "Mercado Mensal".to_string(),
        )
        .unwrap();

        assert_eq!(tx.amount, amount);
        assert_eq!(tx.description, "Mercado Mensal");
        assert_eq!(tx.status, TransactionStatus::Paid);
    }

    #[test]
    fn test_mark_as_paid() {
        let user_id = UserId::generate();
        let account_id = AccountId::generate();
        let category_id = CategoryId::generate();
        let amount = Money::new(dec!(89.90)).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();

        let mut tx = Transaction::new_full(
            user_id,
            account_id,
            category_id,
            TransactionKind::Expense,
            amount,
            date,
            "Conta de Luz".to_string(),
            TransactionStatus::Pending,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(tx.status, TransactionStatus::Pending);

        let new_date = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        tx.mark_as_paid(Some(new_date)).unwrap();
        assert_eq!(tx.status, TransactionStatus::Paid);
        assert_eq!(tx.date, new_date);

        // Trying to mark as paid again fails
        let err = tx.mark_as_paid(None).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn test_split_installments_rounding_remainder_to_first() {
        // R$ 100,00 in 3 installments -> 33.34, 33.33, 33.33
        let total = Money::new(dec!(100.00)).unwrap();
        let parts = split_installments(total, 3).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], Money::new(dec!(33.34)).unwrap());
        assert_eq!(parts[1], Money::new(dec!(33.33)).unwrap());
        assert_eq!(parts[2], Money::new(dec!(33.33)).unwrap());

        let sum: rust_decimal::Decimal = parts.iter().map(|p| p.as_decimal()).sum();
        assert_eq!(sum, dec!(100.00));
    }

    #[test]
    fn test_calculate_installment_dates_d05() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let dates = calculate_installment_dates(start, 4);
        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(), // Feb 28
                NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(), // Apr 30
            ]
        );
    }
}
