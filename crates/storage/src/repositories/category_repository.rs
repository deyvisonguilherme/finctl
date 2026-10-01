use crate::errors::StorageError;
use chrono::{DateTime, Utc};
use domain::{Category, CategoryId, TransactionKind, UserId};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct CategoryRepository;

impl CategoryRepository {
    pub async fn create(pool: &PgPool, category: &Category) -> Result<(), StorageError> {
        let parent_uuid = category.parent_id.map(|id| id.as_uuid());

        let res = sqlx::query(
            r#"
            INSERT INTO categories (id, user_id, name, kind, parent_id, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(category.id.as_uuid())
        .bind(category.user_id.as_uuid())
        .bind(&category.name)
        .bind(category.kind.as_str())
        .bind(parent_uuid)
        .bind(category.created_at)
        .execute(pool)
        .await;

        match res {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::UniqueViolation(format!(
                    "Já existe uma categoria com o nome '{}' sob o mesmo pai.",
                    category.name
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub async fn list_by_user(
        pool: &PgPool,
        user_id: UserId,
    ) -> Result<Vec<Category>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, name, kind, parent_id, created_at
            FROM categories
            WHERE user_id = $1
            ORDER BY name ASC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut categories = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
            let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
            let name: String = row.try_get("name").map_err(StorageError::Database)?;
            let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
            let parent_id_opt: Option<Uuid> =
                row.try_get("parent_id").map_err(StorageError::Database)?;
            let created_at: DateTime<Utc> =
                row.try_get("created_at").map_err(StorageError::Database)?;

            let kind: TransactionKind = kind_str
                .parse()
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

            categories.push(Category {
                id: CategoryId::new(id),
                user_id: UserId::new(u_id),
                name,
                kind,
                parent_id: parent_id_opt.map(CategoryId::new),
                created_at,
            });
        }

        Ok(categories)
    }

    pub async fn find_by_id_or_name(
        pool: &PgPool,
        user_id: UserId,
        query: &str,
    ) -> Result<Option<Category>, StorageError> {
        let is_uuid = Uuid::parse_str(query.trim()).ok();

        let row = if let Some(uuid_val) = is_uuid {
            sqlx::query(
                r#"
                SELECT id, user_id, name, kind, parent_id, created_at
                FROM categories
                WHERE user_id = $1 AND (id = $2 OR LOWER(name) = LOWER($3))
                LIMIT 1
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(uuid_val)
            .bind(query.trim())
            .fetch_optional(pool)
            .await
            .map_err(StorageError::Database)?
        } else {
            sqlx::query(
                r#"
                SELECT id, user_id, name, kind, parent_id, created_at
                FROM categories
                WHERE user_id = $1 AND LOWER(name) = LOWER($2)
                LIMIT 1
                "#,
            )
            .bind(user_id.as_uuid())
            .bind(query.trim())
            .fetch_optional(pool)
            .await
            .map_err(StorageError::Database)?
        };

        if let Some(row) = row {
            let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
            let u_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
            let name: String = row.try_get("name").map_err(StorageError::Database)?;
            let kind_str: String = row.try_get("kind").map_err(StorageError::Database)?;
            let parent_id_opt: Option<Uuid> =
                row.try_get("parent_id").map_err(StorageError::Database)?;
            let created_at: DateTime<Utc> =
                row.try_get("created_at").map_err(StorageError::Database)?;

            let kind: TransactionKind = kind_str
                .parse()
                .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

            Ok(Some(Category {
                id: CategoryId::new(id),
                user_id: UserId::new(u_id),
                name,
                kind,
                parent_id: parent_id_opt.map(CategoryId::new),
                created_at,
            }))
        } else {
            Ok(None)
        }
    }
}
