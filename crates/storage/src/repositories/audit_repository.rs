use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{AuditAction, AuditEntry};
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

#[derive(Debug, Default, Clone)]
pub struct AuditFilter {
    pub table_name: Option<String>,
    pub row_id: Option<Uuid>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

pub struct AuditRepository;

impl AuditRepository {
    pub async fn list(pool: &PgPool, filter: AuditFilter) -> Result<Vec<AuditEntry>, StorageError> {
        let mut builder = QueryBuilder::new(
            "SELECT id, table_name, row_id, action, old, new, changed_at, actor FROM audit_log WHERE 1=1 ",
        );

        if let Some(table) = filter.table_name {
            builder.push(" AND table_name = ");
            builder.push_bind(table);
        }

        if let Some(row_id) = filter.row_id {
            builder.push(" AND row_id = ");
            builder.push_bind(row_id);
        }

        if let Some(since) = filter.since {
            builder.push(" AND changed_at >= ");
            builder.push_bind(since);
        }

        builder.push(" ORDER BY changed_at DESC, id DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ");
            builder.push_bind(limit);
        }

        let rows = builder
            .build()
            .fetch_all(pool)
            .await
            .map_err(StorageError::Database)?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let id: i64 = row.try_get("id").map_err(StorageError::Database)?;
            let table_name: String = row.try_get("table_name").map_err(StorageError::Database)?;
            let row_id: Uuid = row.try_get("row_id").map_err(StorageError::Database)?;
            let action_str: String = row.try_get("action").map_err(StorageError::Database)?;
            let action = action_str.parse::<AuditAction>().map_err(|e| {
                StorageError::Database(sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                ))))
            })?;
            let old: Option<serde_json::Value> =
                row.try_get("old").map_err(StorageError::Database)?;
            let new: Option<serde_json::Value> =
                row.try_get("new").map_err(StorageError::Database)?;
            let changed_at: DateTime<Utc> =
                row.try_get("changed_at").map_err(StorageError::Database)?;
            let actor: String = row.try_get("actor").map_err(StorageError::Database)?;

            entries.push(AuditEntry {
                id,
                table_name,
                row_id,
                action,
                old,
                new,
                changed_at,
                actor,
            });
        }

        Ok(entries)
    }
}
