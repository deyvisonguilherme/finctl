use crate::errors::AppError;
use domain::AuditEntry;
use sqlx::PgPool;
pub use storage::AuditFilter;
use storage::AuditRepository;

pub struct AuditService<'a> {
    pool: &'a PgPool,
}

impl<'a> AuditService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>, AppError> {
        AuditRepository::list(self.pool, filter)
            .await
            .map_err(AppError::from)
    }
}
