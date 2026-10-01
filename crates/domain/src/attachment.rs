use crate::errors::DomainError;
use crate::types::{AttachmentId, TransactionId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: AttachmentId,
    pub user_id: UserId,
    pub transaction_id: TransactionId,
    pub uri: String,
    pub sha256: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Attachment {
    pub fn new(
        id: AttachmentId,
        user_id: UserId,
        transaction_id: TransactionId,
        uri: String,
        sha256: Option<String>,
        note: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        let trimmed_uri = uri.trim();
        if trimmed_uri.is_empty() {
            return Err(DomainError::Validation(
                "A URI do anexo não pode ser vazia.".to_string(),
            ));
        }

        let cleaned_sha256 = sha256.and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        });

        let cleaned_note = note.and_then(|n| {
            let t = n.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        });

        Ok(Self {
            id,
            user_id,
            transaction_id,
            uri: trimmed_uri.to_string(),
            sha256: cleaned_sha256,
            note: cleaned_note,
            created_at,
        })
    }
}
