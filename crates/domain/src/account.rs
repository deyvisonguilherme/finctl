use crate::errors::DomainError;
use crate::money::Money;
use crate::types::{AccountId, AccountKind, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub user_id: UserId,
    pub name: String,
    pub kind: AccountKind,
    pub initial_balance: Money,
    pub closing_day: Option<u8>,
    pub due_day: Option<u8>,
    pub credit_limit: Option<Money>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Account {
    pub fn new(
        user_id: UserId,
        name: String,
        kind: AccountKind,
        initial_balance: Money,
    ) -> Result<Self, DomainError> {
        Self::new_full(user_id, name, kind, initial_balance, None, None, None)
    }

    pub fn new_credit_card(
        user_id: UserId,
        name: String,
        closing_day: u8,
        due_day: u8,
        credit_limit: Option<Money>,
    ) -> Result<Self, DomainError> {
        if !(1..=31).contains(&closing_day) {
            return Err(DomainError::Validation(
                "O dia de fechamento deve estar entre 1 e 31.".to_string(),
            ));
        }
        if !(1..=31).contains(&due_day) {
            return Err(DomainError::Validation(
                "O dia de vencimento deve estar entre 1 e 31.".to_string(),
            ));
        }
        Self::new_full(
            user_id,
            name,
            AccountKind::CreditCard,
            Money::ZERO,
            Some(closing_day),
            Some(due_day),
            credit_limit,
        )
    }

    pub fn new_full(
        user_id: UserId,
        name: String,
        kind: AccountKind,
        initial_balance: Money,
        closing_day: Option<u8>,
        due_day: Option<u8>,
        credit_limit: Option<Money>,
    ) -> Result<Self, DomainError> {
        let trimmed_name = name.trim().to_string();
        if trimmed_name.is_empty() {
            return Err(DomainError::Validation(
                "O nome da conta não pode ser vazio.".to_string(),
            ));
        }
        if trimmed_name.len() > 100 {
            return Err(DomainError::Validation(
                "O nome da conta não pode ter mais de 100 caracteres.".to_string(),
            ));
        }

        if let Some(day) = closing_day {
            if !(1..=31).contains(&day) {
                return Err(DomainError::Validation(
                    "O dia de fechamento deve estar entre 1 e 31.".to_string(),
                ));
            }
        }

        if let Some(day) = due_day {
            if !(1..=31).contains(&day) {
                return Err(DomainError::Validation(
                    "O dia de vencimento deve estar entre 1 e 31.".to_string(),
                ));
            }
        }

        let now = Utc::now();
        Ok(Self {
            id: AccountId::generate(),
            user_id,
            name: trimmed_name,
            kind,
            initial_balance,
            closing_day,
            due_day,
            credit_limit,
            created_at: now,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_create_account_success() {
        let user_id = UserId::generate();
        let balance = Money::from_decimal_non_negative(dec!(1500.00)).unwrap();
        let acc = Account::new(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            balance,
        )
        .unwrap();
        assert_eq!(acc.name, "Nubank");
        assert_eq!(acc.kind, AccountKind::Checking);
        assert_eq!(acc.initial_balance, balance);
    }

    #[test]
    fn test_create_credit_card_account() {
        let user_id = UserId::generate();
        let limit = Money::new(dec!(5000.00)).unwrap();
        let card =
            Account::new_credit_card(user_id, "Cartão Nubank".to_string(), 25, 5, Some(limit))
                .unwrap();
        assert_eq!(card.kind, AccountKind::CreditCard);
        assert_eq!(card.closing_day, Some(25));
        assert_eq!(card.due_day, Some(5));
        assert_eq!(card.credit_limit, Some(limit));
    }

    #[test]
    fn test_create_account_invalid_name() {
        let user_id = UserId::generate();
        let balance = Money::from_decimal_non_negative(dec!(0.00)).unwrap();
        assert!(Account::new(user_id, "   ".to_string(), AccountKind::Checking, balance).is_err());
    }
}
