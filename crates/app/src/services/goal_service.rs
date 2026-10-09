use crate::errors::AppError;
use chrono::{Duration, NaiveDate, Utc};
use domain::{AccountKind, Goal, GoalContribution, GoalProgress, GoalStatus, Money, UserId};
use rust_decimal::Decimal;
use sqlx::PgPool;
use storage::{AccountRepository, GoalRepository};

pub struct CreateGoalInput {
    pub user_id: UserId,
    pub name: String,
    pub target_amount: Money,
    pub target_date: Option<NaiveDate>,
    pub account_query: Option<String>,
}

pub struct EditGoalInput {
    pub user_id: UserId,
    pub identifier: String,
    pub name: Option<String>,
    pub target_amount: Option<Money>,
    pub target_date: Option<Option<NaiveDate>>,
    pub reopen: bool,
}

pub struct AddContributionInput {
    pub user_id: UserId,
    pub goal_identifier: String,
    pub amount: Money,
    pub date: NaiveDate,
    pub note: Option<String>,
}

pub struct GoalService<'a> {
    pool: &'a PgPool,
}

impl<'a> GoalService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_goal(&self, input: CreateGoalInput) -> Result<Goal, AppError> {
        let account_id = if let Some(ref acc_q) = input.account_query {
            let account = AccountRepository::find_by_id_or_name(self.pool, input.user_id, acc_q)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Conta '{acc_q}' vinculada não encontrada."))
                })?;

            if account.kind == AccountKind::CreditCard {
                return Err(AppError::Validation(
                    "Metas de economia não podem ser vinculadas a contas do tipo Cartão de Crédito."
                        .to_string(),
                ));
            }
            Some(account.id)
        } else {
            None
        };

        let mut goal = Goal::new(
            input.user_id,
            input.name,
            input.target_amount,
            input.target_date,
            account_id,
        )?;

        // Se vinculada a uma conta, verifica se o saldo já atinge a meta
        if let Some(acc_id) = goal.account_id {
            let balance =
                GoalRepository::get_account_balance(self.pool, input.user_id, acc_id).await?;
            if balance >= goal.target_amount.as_decimal() {
                goal.mark_completed(Utc::now());
            }
        }

        GoalRepository::create(self.pool, &goal).await?;
        Ok(goal)
    }

    pub async fn list_goals(
        &self,
        user_id: UserId,
        include_completed: bool,
    ) -> Result<Vec<GoalProgress>, AppError> {
        let goals = GoalRepository::list_by_user(self.pool, user_id, include_completed).await?;
        let today = Utc::now().date_naive();

        let mut progress_list = Vec::with_capacity(goals.len());
        for goal in goals {
            let progress = self.calculate_progress_for_goal(goal, today).await?;
            // Se include_completed for falso mas a meta acabou de atingir o alvo no cálculo,
            // ela não deve aparecer na listagem padrão
            if include_completed || !progress.is_completed {
                progress_list.push(progress);
            }
        }

        Ok(progress_list)
    }

    pub async fn get_goal_progress(
        &self,
        user_id: UserId,
        identifier: &str,
    ) -> Result<GoalProgress, AppError> {
        let goal = GoalRepository::find_by_id_or_name(self.pool, user_id, identifier)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Meta '{identifier}' não encontrada.")))?;

        let today = Utc::now().date_naive();
        self.calculate_progress_for_goal(goal, today).await
    }

    pub async fn add_contribution(
        &self,
        input: AddContributionInput,
    ) -> Result<GoalContribution, AppError> {
        let mut goal =
            GoalRepository::find_by_id_or_name(self.pool, input.user_id, &input.goal_identifier)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Meta '{}' não encontrada.", input.goal_identifier))
                })?;

        if goal.is_account_linked() {
            return Err(AppError::Validation(format!(
                "A meta '{}' está vinculada a uma conta bancária. Aportes manuais não são permitidos para metas vinculadas.",
                goal.name
            )));
        }

        if goal.status() == GoalStatus::Completed {
            return Err(AppError::Validation(format!(
                "A meta '{}' já foi concluída. Reabra a meta com 'goal edit --reopen' se desejar adicionar novos aportes.",
                goal.name
            )));
        }

        let contribution =
            GoalContribution::new(goal.id, input.user_id, input.amount, input.date, input.note)?;

        GoalRepository::add_contribution(self.pool, &contribution).await?;

        // Verificar se completou a meta
        let total = GoalRepository::get_manual_goal_sum(self.pool, input.user_id, goal.id).await?;
        if total >= goal.target_amount.as_decimal() {
            goal.mark_completed(Utc::now());
            GoalRepository::update(self.pool, &goal).await?;
        }

        Ok(contribution)
    }

    pub async fn edit_goal(&self, input: EditGoalInput) -> Result<Goal, AppError> {
        let mut goal =
            GoalRepository::find_by_id_or_name(self.pool, input.user_id, &input.identifier)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Meta '{}' não encontrada.", input.identifier))
                })?;

        if let Some(name) = input.name {
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::Validation(
                    "O nome da meta não pode ser vazio.".to_string(),
                ));
            }
            goal.name = trimmed;
        }

        if let Some(target) = input.target_amount {
            if target.as_decimal() <= Decimal::ZERO {
                return Err(AppError::Validation(
                    "O valor alvo da meta deve ser maior que zero.".to_string(),
                ));
            }
            goal.target_amount = target;
        }

        if let Some(date_opt) = input.target_date {
            goal.target_date = date_opt;
        }

        if input.reopen {
            goal.reopen();
        }

        goal.updated_at = Utc::now();
        GoalRepository::update(self.pool, &goal).await?;

        Ok(goal)
    }

    pub async fn delete_goal(&self, user_id: UserId, identifier: &str) -> Result<Goal, AppError> {
        let goal = GoalRepository::find_by_id_or_name(self.pool, user_id, identifier)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Meta '{identifier}' não encontrada.")))?;

        GoalRepository::soft_delete(self.pool, user_id, goal.id).await?;
        Ok(goal)
    }

    async fn calculate_progress_for_goal(
        &self,
        mut goal: Goal,
        today: NaiveDate,
    ) -> Result<GoalProgress, AppError> {
        let ninety_days_ago = today - Duration::days(90);

        let (current_amount, recent_rate, contributions) = if let Some(acc_id) = goal.account_id {
            let balance_dec =
                GoalRepository::get_account_balance(self.pool, goal.user_id, acc_id).await?;
            let current = Money::from_decimal_non_negative(balance_dec.max(Decimal::ZERO))
                .unwrap_or(Money::ZERO);

            let earliest =
                GoalRepository::get_account_created_at_or_first_tx(self.pool, goal.user_id, acc_id)
                    .await?;

            let rate = match earliest {
                Some(earliest_date) if (today - earliest_date).num_days() >= 30 => {
                    let net_flow = GoalRepository::get_account_net_flow_since(
                        self.pool,
                        goal.user_id,
                        acc_id,
                        ninety_days_ago,
                    )
                    .await?;
                    if net_flow > Decimal::ZERO {
                        let avg_rate = (net_flow / Decimal::from(3)).round_dp(2);
                        Money::from_decimal_non_negative(avg_rate).ok()
                    } else {
                        None
                    }
                }
                _ => None,
            };

            (current, rate, Vec::new())
        } else {
            let total_dec =
                GoalRepository::get_manual_goal_sum(self.pool, goal.user_id, goal.id).await?;
            let current = Money::from_decimal_non_negative(total_dec.max(Decimal::ZERO))
                .unwrap_or(Money::ZERO);

            let contribs =
                GoalRepository::list_contributions(self.pool, goal.user_id, goal.id).await?;

            let earliest_contrib =
                GoalRepository::get_earliest_contribution_date(self.pool, goal.user_id, goal.id)
                    .await?;
            let earliest = match earliest_contrib {
                Some(ec) => ec.min(goal.created_at.date_naive()),
                None => goal.created_at.date_naive(),
            };

            let rate = if (today - earliest).num_days() >= 30 {
                let sum_90d = GoalRepository::get_manual_contributions_sum_since(
                    self.pool,
                    goal.user_id,
                    goal.id,
                    ninety_days_ago,
                )
                .await?;
                if sum_90d > Decimal::ZERO {
                    let avg_rate = (sum_90d / Decimal::from(3)).round_dp(2);
                    Money::from_decimal_non_negative(avg_rate).ok()
                } else {
                    None
                }
            } else {
                None
            };

            (current, rate, contribs)
        };

        // Se o valor atingiu a meta e ela ainda não estava com completed_at registrado
        if current_amount.as_decimal() >= goal.target_amount.as_decimal()
            && goal.completed_at.is_none()
        {
            goal.mark_completed(Utc::now());
            GoalRepository::update(self.pool, &goal).await?;
        }

        let progress =
            GoalProgress::calculate(goal, current_amount, recent_rate, today, contributions);
        Ok(progress)
    }
}
