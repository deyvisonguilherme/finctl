use crate::errors::AppError;
use domain::{Tag, TagWithUsage, TransactionId, UserId};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use storage::{TagRepository, TransactionRepository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteTagSummary {
    pub tag_name: String,
    pub usage_count: i64,
    pub deleted: bool,
}

pub struct TagService<'a> {
    pool: &'a PgPool,
}

impl<'a> TagService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn add_tag(&self, user_id: UserId, name: String) -> Result<Tag, AppError> {
        let tag = TagRepository::get_or_create(self.pool, user_id, &name).await?;
        Ok(tag)
    }

    pub async fn list_tags(&self, user_id: UserId) -> Result<Vec<TagWithUsage>, AppError> {
        let tags = TagRepository::list_with_usage(self.pool, user_id).await?;
        Ok(tags)
    }

    pub async fn delete_tag(
        &self,
        user_id: UserId,
        query: &str,
        force: bool,
    ) -> Result<DeleteTagSummary, AppError> {
        let tag = TagRepository::find_by_id_or_name(self.pool, user_id, query)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tag '{query}' não encontrada.")))?;

        let usage_count = TagRepository::count_usage(self.pool, user_id, tag.id).await?;

        if usage_count > 0 && !force {
            return Err(AppError::Validation(format!(
                "A tag '{}' está associada a {} lançamento(s). Use --yes ou --force para confirmar a exclusão.",
                tag.name, usage_count
            )));
        }

        let deleted = TagRepository::delete(self.pool, user_id, tag.id).await?;

        Ok(DeleteTagSummary {
            tag_name: tag.name,
            usage_count,
            deleted,
        })
    }

    pub async fn tag_transaction(
        &self,
        user_id: UserId,
        tx_query: &str,
        tag_names: Vec<String>,
    ) -> Result<Vec<String>, AppError> {
        let tx_id = TransactionId::from_str(tx_query).map_err(|_| {
            AppError::Validation(format!("ID de lançamento inválido '{tx_query}'."))
        })?;

        let tx = TransactionRepository::find_by_id(self.pool, user_id, tx_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Lançamento com ID '{tx_query}' não encontrado."))
            })?;

        if tag_names.is_empty() {
            return Err(AppError::Validation(
                "Informe pelo menos uma tag para associar ao lançamento.".to_string(),
            ));
        }

        let mut tag_ids = Vec::with_capacity(tag_names.len());
        for name in &tag_names {
            let clean_name = name.trim().trim_start_matches('#');
            if !clean_name.is_empty() {
                let tag = TagRepository::get_or_create(self.pool, user_id, clean_name).await?;
                tag_ids.push(tag.id);
            }
        }

        TagRepository::add_tags_to_transaction(self.pool, tx.id, &tag_ids).await?;

        let updated_tags = TagRepository::get_tags_for_transaction(self.pool, tx.id).await?;
        Ok(updated_tags.into_iter().map(|t| t.name).collect())
    }
}
