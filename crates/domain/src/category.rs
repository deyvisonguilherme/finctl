use crate::errors::DomainError;
use crate::types::{CategoryId, TransactionKind, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub user_id: UserId,
    pub name: String,
    pub kind: TransactionKind,
    pub parent_id: Option<CategoryId>,
    pub created_at: DateTime<Utc>,
}

impl Category {
    pub fn new(
        user_id: UserId,
        name: String,
        kind: TransactionKind,
        parent_id: Option<CategoryId>,
    ) -> Result<Self, DomainError> {
        let trimmed_name = name.trim().to_string();
        if trimmed_name.is_empty() {
            return Err(DomainError::Validation(
                "O nome da categoria não pode ser vazio.".to_string(),
            ));
        }
        if trimmed_name.len() > 100 {
            return Err(DomainError::Validation(
                "O nome da categoria não pode ter mais de 100 caracteres.".to_string(),
            ));
        }

        Ok(Self {
            id: CategoryId::generate(),
            user_id,
            name: trimmed_name,
            kind,
            parent_id,
            created_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_creation() {
        let user_id = UserId::generate();
        let cat = Category::new(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .unwrap();
        assert_eq!(cat.name, "Alimentação");
        assert_eq!(cat.kind, TransactionKind::Expense);
        assert!(cat.parent_id.is_none());
    }

    #[test]
    fn test_category_empty_name() {
        let user_id = UserId::generate();
        assert!(Category::new(user_id, "  ".to_string(), TransactionKind::Income, None).is_err());
    }
}
