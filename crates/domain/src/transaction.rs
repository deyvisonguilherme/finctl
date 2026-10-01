use crate::errors::DomainError;
use crate::money::Money;
use crate::types::{
    AccountId, CategoryId, TransactionId, TransactionKind, TransactionStatus, UserId,
};
use chrono::{DateTime, NaiveDate, Utc};
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
        })
    }
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
}
