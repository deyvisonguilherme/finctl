use crate::errors::DomainError;
use crate::money::Money;
use crate::types::{BudgetId, CategoryId, UserId};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetIndicator {
    Ok,
    Warning,
    Exceeded,
}

impl BudgetIndicator {
    pub fn from_percentage(pct: Decimal) -> Self {
        if pct > Decimal::from(100) {
            Self::Exceeded
        } else if pct >= Decimal::from(80) {
            Self::Warning
        } else {
            Self::Ok
        }
    }

    pub fn display_pt_br(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "ALERTA (≥ 80%)",
            Self::Exceeded => "ESTOURADO",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    pub id: BudgetId,
    pub user_id: UserId,
    pub category_id: CategoryId,
    pub amount: Money,
    pub month: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Budget {
    pub fn new(
        user_id: UserId,
        category_id: CategoryId,
        amount: Money,
        month: Option<String>,
    ) -> Result<Self, DomainError> {
        let validated_month = if let Some(m) = month {
            let trimmed = m.trim().to_string();
            validate_month_format(&trimmed)?;
            Some(trimmed)
        } else {
            None
        };

        let now = Utc::now();
        Ok(Self {
            id: BudgetId::generate(),
            user_id,
            category_id,
            amount,
            month: validated_month,
            created_at: now,
            updated_at: now,
        })
    }
}

fn validate_month_format(month_str: &str) -> Result<(), DomainError> {
    let parts: Vec<&str> = month_str.split('-').collect();
    if parts.len() != 2 || parts[0].len() != 4 || parts[1].len() != 2 {
        return Err(DomainError::Validation(format!(
            "Mês de orçamento inválido '{month_str}'. Use o formato AAAA-MM (ex: 2026-10)."
        )));
    }

    let year: i32 = parts[0]
        .parse()
        .map_err(|_| DomainError::Validation(format!("Ano inválido '{}'", parts[0])))?;
    let month: u32 = parts[1]
        .parse()
        .map_err(|_| DomainError::Validation(format!("Mês inválido '{}'", parts[1])))?;

    if !(2000..=2100).contains(&year) || !(1..=12).contains(&month) {
        return Err(DomainError::Validation(format!(
            "Mês de orçamento fora do intervalo permitido: '{month_str}'."
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_budget_creation_recurring() {
        let user_id = UserId::generate();
        let cat_id = CategoryId::generate();
        let amount = Money::new(dec!(1500.00)).unwrap();

        let budget = Budget::new(user_id, cat_id, amount, None).unwrap();
        assert_eq!(budget.amount, amount);
        assert_eq!(budget.month, None);
    }

    #[test]
    fn test_budget_creation_monthly_exception() {
        let user_id = UserId::generate();
        let cat_id = CategoryId::generate();
        let amount = Money::new(dec!(2000.00)).unwrap();

        let budget = Budget::new(user_id, cat_id, amount, Some("2026-10".to_string())).unwrap();
        assert_eq!(budget.month, Some("2026-10".to_string()));

        let err = Budget::new(user_id, cat_id, amount, Some("2026/10".to_string())).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn test_budget_indicators() {
        assert_eq!(
            BudgetIndicator::from_percentage(dec!(50.00)),
            BudgetIndicator::Ok
        );
        assert_eq!(
            BudgetIndicator::from_percentage(dec!(79.99)),
            BudgetIndicator::Ok
        );
        assert_eq!(
            BudgetIndicator::from_percentage(dec!(80.00)),
            BudgetIndicator::Warning
        );
        assert_eq!(
            BudgetIndicator::from_percentage(dec!(100.00)),
            BudgetIndicator::Warning
        );
        assert_eq!(
            BudgetIndicator::from_percentage(dec!(100.01)),
            BudgetIndicator::Exceeded
        );
    }
}
