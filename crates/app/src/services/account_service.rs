use crate::errors::AppError;
use domain::{Account, AccountKind, Money, UserId};
use sqlx::PgPool;
use storage::AccountRepository;

pub struct AccountService<'a> {
    pool: &'a PgPool,
}

impl<'a> AccountService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_account(
        &self,
        user_id: UserId,
        name: String,
        kind: AccountKind,
        initial_balance: Money,
    ) -> Result<Account, AppError> {
        let account = Account::new(user_id, name, kind, initial_balance)?;
        AccountRepository::create(self.pool, &account).await?;
        Ok(account)
    }

    pub async fn list_accounts(&self, user_id: UserId) -> Result<Vec<Account>, AppError> {
        let accounts = AccountRepository::list_by_user(self.pool, user_id).await?;
        Ok(accounts)
    }
}
