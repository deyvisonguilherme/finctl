use crate::errors::AppError;
use domain::{Account, AccountKind, Money, UserId};
use sqlx::PgPool;
use storage::AccountRepository;

#[derive(Debug, Clone)]
pub struct CreateAccountInput {
    pub user_id: UserId,
    pub name: String,
    pub kind: AccountKind,
    pub initial_balance: Money,
    pub closing_day: Option<u8>,
    pub due_day: Option<u8>,
    pub credit_limit: Option<Money>,
}

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
        self.create_account_with_input(CreateAccountInput {
            user_id,
            name,
            kind,
            initial_balance,
            closing_day: None,
            due_day: None,
            credit_limit: None,
        })
        .await
    }

    pub async fn create_account_with_input(
        &self,
        input: CreateAccountInput,
    ) -> Result<Account, AppError> {
        let account = Account::new_full(
            input.user_id,
            input.name,
            input.kind,
            input.initial_balance,
            input.closing_day,
            input.due_day,
            input.credit_limit,
        )?;
        AccountRepository::create(self.pool, &account).await?;
        Ok(account)
    }

    pub async fn list_accounts(&self, user_id: UserId) -> Result<Vec<Account>, AppError> {
        let accounts = AccountRepository::list_by_user(self.pool, user_id).await?;
        Ok(accounts)
    }
}
