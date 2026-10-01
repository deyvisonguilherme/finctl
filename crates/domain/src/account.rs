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

        let now = Utc::now();
        Ok(Self {
            id: AccountId::generate(),
            user_id,
            name: trimmed_name,
            kind,
            initial_balance,
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
    fn test_create_account_invalid_name() {
        let user_id = UserId::generate();
        let balance = Money::from_decimal_non_negative(dec!(0.00)).unwrap();
        assert!(Account::new(user_id, "   ".to_string(), AccountKind::Checking, balance).is_err());
    }
}
