use crate::errors::DomainError;
use crate::types::{TagId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub user_id: UserId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Tag {
    pub fn new(
        id: TagId,
        user_id: UserId,
        name: String,
        created_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(DomainError::Validation(
                "O nome da tag não pode ser vazio.".to_string(),
            ));
        }
        if trimmed.len() > 50 {
            return Err(DomainError::Validation(
                "O nome da tag não pode ter mais de 50 caracteres.".to_string(),
            ));
        }

        Ok(Self {
            id,
            user_id,
            name: trimmed.to_string(),
            created_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagWithUsage {
    pub id: TagId,
    pub name: String,
    pub usage_count: i64,
    pub created_at: DateTime<Utc>,
}
