use crate::errors::AppError;
use chrono::Utc;
use domain::{Attachment, AttachmentId, TransactionId, UserId};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use storage::{AttachmentRepository, TransactionRepository};

pub struct AttachmentService<'a> {
    pool: &'a PgPool,
}

impl<'a> AttachmentService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn attach(
        &self,
        user_id: UserId,
        tx_query: &str,
        path_or_uri: &str,
        note: Option<String>,
    ) -> Result<Attachment, AppError> {
        let tx_id = TransactionId::from_str(tx_query).map_err(|_| {
            AppError::Validation(format!("ID de lançamento inválido '{tx_query}'."))
        })?;

        let _tx = TransactionRepository::find_by_id(self.pool, user_id, tx_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Lançamento com ID '{tx_query}' não encontrado."))
            })?;

        let trimmed_uri = path_or_uri.trim();
        if trimmed_uri.is_empty() {
            return Err(AppError::Validation(
                "O caminho ou URL do anexo não pode ser vazio.".to_string(),
            ));
        }

        let is_remote_url = trimmed_uri.starts_with("http://")
            || trimmed_uri.starts_with("https://")
            || trimmed_uri.starts_with("ftp://");

        let (final_uri, sha256_hash) = if is_remote_url {
            (trimmed_uri.to_string(), None)
        } else {
            let local_path_str = trimmed_uri.trim_start_matches("file://");
            let local_path = Path::new(local_path_str);

            if !local_path.exists() {
                return Err(AppError::Validation(format!(
                    "Arquivo local '{}' não encontrado.",
                    local_path_str
                )));
            }

            let mut file = File::open(local_path).map_err(|e| {
                AppError::Validation(format!(
                    "Erro ao abrir arquivo local '{}': {e}",
                    local_path_str
                ))
            })?;

            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 8192];
            loop {
                let bytes_read = file
                    .read(&mut buffer)
                    .map_err(|e| AppError::Validation(format!("Erro ao ler arquivo: {e}")))?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
            }

            let hash_result = hasher.finalize();
            let hash_hex = format!("{:x}", hash_result);

            (local_path_str.to_string(), Some(hash_hex))
        };

        let attachment = Attachment::new(
            AttachmentId::generate(),
            user_id,
            tx_id,
            final_uri,
            sha256_hash,
            note,
            Utc::now(),
        )?;

        AttachmentRepository::create(self.pool, &attachment).await?;

        Ok(attachment)
    }

    pub async fn list_attachments(
        &self,
        user_id: UserId,
        tx_query: &str,
    ) -> Result<Vec<Attachment>, AppError> {
        let tx_id = TransactionId::from_str(tx_query).map_err(|_| {
            AppError::Validation(format!("ID de lançamento inválido '{tx_query}'."))
        })?;

        let attachments =
            AttachmentRepository::list_by_transaction(self.pool, user_id, tx_id).await?;
        Ok(attachments)
    }
}
