use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{Tag, TagId, TagWithUsage, TransactionId, UserId};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

pub struct TagRepository;

impl TagRepository {
    pub async fn create(pool: &PgPool, tag: &Tag) -> Result<(), StorageError> {
        let res = sqlx::query(
            r#"
            INSERT INTO tags (id, user_id, name, created_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(tag.id.as_uuid())
        .bind(tag.user_id.as_uuid())
        .bind(&tag.name)
        .bind(tag.created_at)
        .execute(pool)
        .await;

        match res {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::UniqueViolation(format!(
                    "A tag '{}' já existe para este usuário.",
                    tag.name
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub async fn find_by_id(
        pool: &PgPool,
        user_id: UserId,
        id: TagId,
    ) -> Result<Option<Tag>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, name, created_at
            FROM tags
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        row.map(map_tag_row).transpose()
    }

    pub async fn find_by_name(
        pool: &PgPool,
        user_id: UserId,
        name: &str,
    ) -> Result<Option<Tag>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, name, created_at
            FROM tags
            WHERE user_id = $1 AND LOWER(name) = LOWER($2)
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(name.trim())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        row.map(map_tag_row).transpose()
    }

    pub async fn find_by_id_or_name(
        pool: &PgPool,
        user_id: UserId,
        query: &str,
    ) -> Result<Option<Tag>, StorageError> {
        let trimmed = query.trim();
        if let Ok(id) = TagId::from_str(trimmed) {
            if let Some(tag) = Self::find_by_id(pool, user_id, id).await? {
                return Ok(Some(tag));
            }
        }
        Self::find_by_name(pool, user_id, trimmed).await
    }

    pub async fn get_or_create(
        pool: &PgPool,
        user_id: UserId,
        name: &str,
    ) -> Result<Tag, StorageError> {
        let trimmed = name.trim();
        if let Some(existing) = Self::find_by_name(pool, user_id, trimmed).await? {
            return Ok(existing);
        }

        let new_tag = Tag::new(TagId::generate(), user_id, trimmed.to_string(), Utc::now())
            .map_err(|e| StorageError::Conversion(e.to_string()))?;

        match Self::create(pool, &new_tag).await {
            Ok(()) => Ok(new_tag),
            Err(StorageError::UniqueViolation(_)) => {
                // If created concurrently, fetch it
                Self::find_by_name(pool, user_id, trimmed)
                    .await?
                    .ok_or_else(|| StorageError::Database(sqlx::Error::RowNotFound))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn list_with_usage(
        pool: &PgPool,
        user_id: UserId,
    ) -> Result<Vec<TagWithUsage>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT t.id, t.name, t.created_at, COUNT(tt.transaction_id) AS usage_count
            FROM tags t
            LEFT JOIN transaction_tags tt ON t.id = tt.tag_id
            WHERE t.user_id = $1
            GROUP BY t.id, t.name, t.created_at
            ORDER BY t.name ASC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
            let name: String = row.try_get("name").map_err(StorageError::Database)?;
            let usage_count: i64 = row.try_get("usage_count").map_err(StorageError::Database)?;
            let created_at: DateTime<Utc> =
                row.try_get("created_at").map_err(StorageError::Database)?;

            list.push(TagWithUsage {
                id: TagId::new(id),
                name,
                usage_count,
                created_at,
            });
        }

        Ok(list)
    }

    pub async fn count_usage(
        pool: &PgPool,
        user_id: UserId,
        id: TagId,
    ) -> Result<i64, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(tt.transaction_id) as count
            FROM transaction_tags tt
            JOIN tags t ON tt.tag_id = t.id
            WHERE t.id = $1 AND t.user_id = $2
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .fetch_one(pool)
        .await
        .map_err(StorageError::Database)?;

        let count: i64 = row.try_get("count").map_err(StorageError::Database)?;
        Ok(count)
    }

    pub async fn delete(pool: &PgPool, user_id: UserId, id: TagId) -> Result<bool, StorageError> {
        let res = sqlx::query(
            r#"
            DELETE FROM tags
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

    pub async fn add_tags_to_transaction(
        pool: &PgPool,
        transaction_id: TransactionId,
        tag_ids: &[TagId],
    ) -> Result<(), StorageError> {
        for tag_id in tag_ids {
            sqlx::query(
                r#"
                INSERT INTO transaction_tags (transaction_id, tag_id)
                VALUES ($1, $2)
                ON CONFLICT (transaction_id, tag_id) DO NOTHING
                "#,
            )
            .bind(transaction_id.as_uuid())
            .bind(tag_id.as_uuid())
            .execute(pool)
            .await
            .map_err(StorageError::Database)?;
        }
        Ok(())
    }

    pub async fn get_tags_for_transaction(
        pool: &PgPool,
        transaction_id: TransactionId,
    ) -> Result<Vec<Tag>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT t.id, t.user_id, t.name, t.created_at
            FROM tags t
            JOIN transaction_tags tt ON t.id = tt.tag_id
            WHERE tt.transaction_id = $1
            ORDER BY t.name ASC
            "#,
        )
        .bind(transaction_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut tags = Vec::with_capacity(rows.len());
        for row in rows {
            tags.push(map_tag_row(row)?);
        }
        Ok(tags)
    }

    pub async fn get_tags_for_transactions(
        pool: &PgPool,
        tx_ids: &[TransactionId],
    ) -> Result<HashMap<TransactionId, Vec<String>>, StorageError> {
        if tx_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let uuids: Vec<Uuid> = tx_ids.iter().map(|id| id.as_uuid()).collect();

        let rows = sqlx::query(
            r#"
            SELECT tt.transaction_id, t.name
            FROM tags t
            JOIN transaction_tags tt ON t.id = tt.tag_id
            WHERE tt.transaction_id = ANY($1)
            ORDER BY t.name ASC
            "#,
        )
        .bind(&uuids)
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut map: HashMap<TransactionId, Vec<String>> = HashMap::new();
        for row in rows {
            let tx_id: Uuid = row
                .try_get("transaction_id")
                .map_err(StorageError::Database)?;
            let tag_name: String = row.try_get("name").map_err(StorageError::Database)?;

            map.entry(TransactionId::new(tx_id))
                .or_default()
                .push(tag_name);
        }

        Ok(map)
    }
}

fn map_tag_row(row: sqlx::postgres::PgRow) -> Result<Tag, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let user_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let name: String = row.try_get("name").map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;

    Tag::new(TagId::new(id), UserId::new(user_id), name, created_at)
        .map_err(|e| StorageError::Conversion(e.to_string()))
}
