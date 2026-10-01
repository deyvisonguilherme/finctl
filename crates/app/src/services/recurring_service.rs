use crate::errors::AppError;
use chrono::{Local, NaiveDate, Utc};
use domain::{Money, RecurringFrequency, RecurringRule, RecurringRuleId, TransactionKind, UserId};
use sqlx::PgPool;
use storage::{AccountRepository, CategoryRepository, RecurringRepository, RecurringRuleDetails};

pub struct CreateRecurringInput {
    pub user_id: UserId,
    pub account_query: String,
    pub category_query: String,
    pub kind: TransactionKind,
    pub amount: Money,
    pub description: String,
    pub frequency: RecurringFrequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

pub struct EditRecurringInput {
    pub user_id: UserId,
    pub id: RecurringRuleId,
    pub account_query: Option<String>,
    pub category_query: Option<String>,
    pub amount: Option<Money>,
    pub description: Option<String>,
    pub end_date: Option<NaiveDate>,
}

pub struct RecurringService<'a> {
    pool: &'a PgPool,
}

impl<'a> RecurringService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_rule(
        &self,
        input: CreateRecurringInput,
    ) -> Result<RecurringRule, AppError> {
        let account =
            AccountRepository::find_by_id_or_name(self.pool, input.user_id, &input.account_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Conta '{}' não encontrada.", input.account_query))
                })?;

        let category =
            CategoryRepository::find_by_id_or_name(self.pool, input.user_id, &input.category_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Categoria '{}' não encontrada.",
                        input.category_query
                    ))
                })?;

        if category.kind != input.kind {
            return Err(AppError::Validation(format!(
                "A categoria '{}' é do tipo '{}', mas a recorrência é do tipo '{}'.",
                category.name,
                category.kind.display_pt_br(),
                input.kind.display_pt_br()
            )));
        }

        let start_date = input
            .start_date
            .unwrap_or_else(|| Local::now().date_naive());

        let rule = RecurringRule::new(
            input.user_id,
            account.id,
            category.id,
            input.kind,
            input.amount,
            input.description,
            input.frequency,
            input.day_of_month,
            input.day_of_week,
            start_date,
            input.end_date,
        )?;

        RecurringRepository::create(self.pool, &rule).await?;
        Ok(rule)
    }

    pub async fn list_rules(
        &self,
        user_id: UserId,
        active_only: Option<bool>,
    ) -> Result<Vec<RecurringRuleDetails>, AppError> {
        let rules = RecurringRepository::list_by_user(self.pool, user_id, active_only).await?;
        Ok(rules)
    }

    pub async fn edit_rule(&self, input: EditRecurringInput) -> Result<RecurringRule, AppError> {
        let mut rule = RecurringRepository::find_by_id(self.pool, input.user_id, input.id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Regra de recorrência com ID '{}' não encontrada.",
                    input.id
                ))
            })?;

        if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            rule.account_id = acc.id;
        }

        if let Some(ref cat_q) = input.category_query {
            let cat = CategoryRepository::find_by_id_or_name(self.pool, input.user_id, cat_q)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Categoria '{cat_q}' não encontrada."))
                })?;

            if cat.kind != rule.kind {
                return Err(AppError::Validation(format!(
                    "A categoria '{}' é do tipo '{}', mas a recorrência é do tipo '{}'.",
                    cat.name,
                    cat.kind.display_pt_br(),
                    rule.kind.display_pt_br()
                )));
            }
            rule.category_id = cat.id;
        }

        if let Some(amount) = input.amount {
            rule.amount = amount;
        }

        if let Some(description) = input.description {
            let trimmed = description.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::Validation(
                    "A descrição não pode ficar vazia.".to_string(),
                ));
            }
            if trimmed.len() > 255 {
                return Err(AppError::Validation(
                    "A descrição não pode ter mais de 255 caracteres.".to_string(),
                ));
            }
            rule.description = trimmed;
        }

        if let Some(end_date) = input.end_date {
            if end_date < rule.start_date {
                return Err(AppError::Validation(
                    "A data final não pode ser anterior à data inicial.".to_string(),
                ));
            }
            rule.end_date = Some(end_date);
        }

        rule.updated_at = Utc::now();
        RecurringRepository::update(self.pool, &rule).await?;
        Ok(rule)
    }

    pub async fn pause_rule(
        &self,
        user_id: UserId,
        id: RecurringRuleId,
    ) -> Result<RecurringRule, AppError> {
        let mut rule = RecurringRepository::find_by_id(self.pool, user_id, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Regra de recorrência com ID '{id}' não encontrada."
                ))
            })?;

        rule.pause();
        RecurringRepository::update(self.pool, &rule).await?;
        Ok(rule)
    }

    pub async fn resume_rule(
        &self,
        user_id: UserId,
        id: RecurringRuleId,
    ) -> Result<RecurringRule, AppError> {
        let mut rule = RecurringRepository::find_by_id(self.pool, user_id, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Regra de recorrência com ID '{id}' não encontrada."
                ))
            })?;

        rule.resume();
        RecurringRepository::update(self.pool, &rule).await?;
        Ok(rule)
    }

    pub async fn delete_rule(&self, user_id: UserId, id: RecurringRuleId) -> Result<(), AppError> {
        let deleted = RecurringRepository::delete(self.pool, user_id, id).await?;
        if !deleted {
            return Err(AppError::NotFound(format!(
                "Regra de recorrência com ID '{id}' não encontrada."
            )));
        }
        Ok(())
    }
}
