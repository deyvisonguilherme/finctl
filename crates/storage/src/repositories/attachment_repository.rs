use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{Attachment, AttachmentId, TransactionId, UserId};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

pub struct AttachmentRepository;

impl AttachmentRepository {
    pub async fn create(pool: &PgPool, attachment: &Attachment) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO attachments (id, user_id, transaction_id, uri, sha256, note, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(attachment.id.as_uuid())
        .bind(attachment.user_id.as_uuid())
        .bind(attachment.transaction_id.as_uuid())
        .bind(&attachment.uri)
        .bind(&attachment.sha256)
        .bind(&attachment.note)
        .bind(attachment.created_at)
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(())
    }

    pub async fn list_by_transaction(
        pool: &PgPool,
        user_id: UserId,
        transaction_id: TransactionId,
    ) -> Result<Vec<Attachment>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, transaction_id, uri, sha256, note, created_at
            FROM attachments
            WHERE transaction_id = $1 AND user_id = $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(transaction_id.as_uuid())
        .bind(user_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut attachments = Vec::with_capacity(rows.len());
        for row in rows {
            attachments.push(map_attachment_row(row)?);
        }
        Ok(attachments)
    }

    pub async fn get_attachments_for_transactions(
        pool: &PgPool,
        tx_ids: &[TransactionId],
    ) -> Result<HashMap<TransactionId, Vec<Attachment>>, StorageError> {
        if tx_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let uuids: Vec<Uuid> = tx_ids.iter().map(|id| id.as_uuid()).collect();

        let rows = sqlx::query(
            r#"
            SELECT id, user_id, transaction_id, uri, sha256, note, created_at
            FROM attachments
            WHERE transaction_id = ANY($1)
            ORDER BY created_at ASC
            "#,
        )
        .bind(&uuids)
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut map: HashMap<TransactionId, Vec<Attachment>> = HashMap::new();
        for row in rows {
            let att = map_attachment_row(row)?;
            map.entry(att.transaction_id).or_default().push(att);
        }

        Ok(map)
    }

    pub async fn delete(
        pool: &PgPool,
        user_id: UserId,
        id: AttachmentId,
    ) -> Result<bool, StorageError> {
        let res = sqlx::query(
            r#"
            DELETE FROM attachments
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .execute(pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(res.rows_affected() > 0)
    }
}

fn map_attachment_row(row: sqlx::postgres::PgRow) -> Result<Attachment, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let user_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let transaction_id: Uuid = row
        .try_get("transaction_id")
        .map_err(StorageError::Database)?;
    let uri: String = row.try_get("uri").map_err(StorageError::Database)?;
    let sha256: Option<String> = row.try_get("sha256").map_err(StorageError::Database)?;
    let note: Option<String> = row.try_get("note").map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;

    Attachment::new(
        AttachmentId::new(id),
        UserId::new(user_id),
        TransactionId::new(transaction_id),
        uri,
        sha256,
        note,
        created_at,
    )
    .map_err(|e| StorageError::Conversion(e.to_string()))
}
