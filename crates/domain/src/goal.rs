use crate::errors::DomainError;
use crate::money::Money;
use crate::types::{AccountId, ContributionId, GoalId, UserId};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Status de uma meta de economia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoalStatus {
    Active,
    Completed,
}

/// Entidade que representa uma meta de economia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub user_id: UserId,
    pub name: String,
    pub target_amount: Money,
    pub target_date: Option<NaiveDate>,
    pub account_id: Option<AccountId>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Goal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: UserId,
        name: String,
        target_amount: Money,
        target_date: Option<NaiveDate>,
        account_id: Option<AccountId>,
    ) -> Result<Self, DomainError> {
        let trimmed_name = name.trim().to_string();
        if trimmed_name.is_empty() {
            return Err(DomainError::Validation(
                "O nome da meta não pode ser vazio".to_string(),
            ));
        }

        if target_amount.as_decimal() <= Decimal::ZERO {
            return Err(DomainError::Validation(
                "O valor alvo da meta deve ser maior que zero".to_string(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id: GoalId::generate(),
            user_id,
            name: trimmed_name,
            target_amount,
            target_date,
            account_id,
            completed_at: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }

    pub fn status(&self) -> GoalStatus {
        if self.completed_at.is_some() {
            GoalStatus::Completed
        } else {
            GoalStatus::Active
        }
    }

    pub fn is_account_linked(&self) -> bool {
        self.account_id.is_some()
    }

    pub fn mark_completed(&mut self, when: DateTime<Utc>) {
        self.completed_at = Some(when);
        self.updated_at = when;
    }

    pub fn reopen(&mut self) {
        self.completed_at = None;
        self.updated_at = Utc::now();
    }
}

/// Entidade que representa um aporte manual registrado para uma meta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalContribution {
    pub id: ContributionId,
    pub goal_id: GoalId,
    pub user_id: UserId,
    pub amount: Money,
    pub date: NaiveDate,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl GoalContribution {
    pub fn new(
        goal_id: GoalId,
        user_id: UserId,
        amount: Money,
        date: NaiveDate,
        note: Option<String>,
    ) -> Result<Self, DomainError> {
        if amount.as_decimal() <= Decimal::ZERO {
            return Err(DomainError::Validation(
                "O valor do aporte deve ser maior que zero".to_string(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id: ContributionId::generate(),
            goal_id,
            user_id,
            amount,
            date,
            note: note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }
}

/// Informações agregadas de progresso, projeções e métricas de uma meta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalProgress {
    pub goal: Goal,
    pub current_amount: Money,
    pub remaining_amount: Money,
    pub percentage: Decimal,
    pub is_completed: bool,
    /// Aporte mensal necessário até a data alvo (se houver data alvo e meta ativa)
    pub monthly_needed: Option<Money>,
    /// Média mensal calculada dos últimos 3 meses (90 dias), ou None se histórico insuficiente
    pub recent_monthly_rate: Option<Money>,
    /// Data estimada de conclusão com base na média dos últimos 3 meses
    pub estimated_completion_date: Option<NaiveDate>,
    pub contributions: Vec<GoalContribution>,
}

impl GoalProgress {
    /// Calcula o progresso e projeções a partir dos dados atuais.
    pub fn calculate(
        goal: Goal,
        current_amount: Money,
        recent_monthly_rate: Option<Money>,
        as_of_date: NaiveDate,
        contributions: Vec<GoalContribution>,
    ) -> Self {
        let target = goal.target_amount.as_decimal();
        let current = current_amount.as_decimal();

        let is_completed = goal.completed_at.is_some() || current >= target;

        let remaining_dec = if current >= target {
            Decimal::ZERO
        } else {
            target - current
        };
        let remaining_amount =
            Money::from_decimal_non_negative(remaining_dec).unwrap_or(Money::ZERO);

        let percentage = if target > Decimal::ZERO {
            ((current / target) * Decimal::from(100)).round_dp(2)
        } else {
            Decimal::ZERO
        };

        // Aporte mensal necessário até a data alvo
        let monthly_needed = if !is_completed {
            goal.target_date.and_then(|target_date| {
                if target_date <= as_of_date {
                    // Já venceu ou vence hoje: necessita do valor restante integral
                    Some(remaining_amount)
                } else {
                    let months_left = months_difference(as_of_date, target_date).max(1);
                    let needed_dec = (remaining_dec / Decimal::from(months_left)).round_dp(2);
                    Money::from_decimal_non_negative(needed_dec).ok()
                }
            })
        } else {
            None
        };

        // Data estimada de conclusão com base na média dos últimos 3 meses
        let estimated_completion_date = if is_completed {
            None
        } else {
            recent_monthly_rate.and_then(|rate| {
                let rate_dec = rate.as_decimal();
                if rate_dec > Decimal::ZERO && remaining_dec > Decimal::ZERO {
                    let months_needed = (remaining_dec / rate_dec).ceil().to_u32().unwrap_or(0);
                    add_months(as_of_date, months_needed)
                } else {
                    None
                }
            })
        };

        Self {
            goal,
            current_amount,
            remaining_amount,
            percentage,
            is_completed,
            monthly_needed,
            recent_monthly_rate,
            estimated_completion_date,
            contributions,
        }
    }
}

/// Calcula a quantidade aproximada de meses entre duas datas (mês final - mês inicial).
pub fn months_difference(from: NaiveDate, to: NaiveDate) -> u32 {
    let years = to.year() - from.year();
    let months = to.month() as i32 - from.month() as i32;
    let total_months = years * 12 + months;
    if total_months <= 0 {
        1
    } else {
        total_months as u32
    }
}

/// Adiciona `months` meses a uma data respeitando D-05 (último dia do mês se ultrapassar).
pub fn add_months(date: NaiveDate, months: u32) -> Option<NaiveDate> {
    if months == 0 {
        return Some(date);
    }

    let mut year = date.year();
    let mut month = date.month() + months;

    while month > 12 {
        year += 1;
        month -= 12;
    }

    let day = date.day();
    // Ajustar para o último dia válido do mês se o dia exceder o tamanho do mês (D-05)
    let max_day = last_day_of_month(year, month);
    let valid_day = day.min(max_day);

    NaiveDate::from_ymd_opt(year, month, valid_day)
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    next_month_first.pred_opt().unwrap().day()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_goal_creation_and_validation() {
        let user_id = UserId::generate();
        let target = Money::from_decimal_non_negative(dec!(5000.00)).unwrap();

        let goal = Goal::new(user_id, "Carro Novo".to_string(), target, None, None).unwrap();
        assert_eq!(goal.name, "Carro Novo");
        assert_eq!(goal.target_amount, target);
        assert_eq!(goal.status(), GoalStatus::Active);

        // Nome vazio deve falhar
        assert!(Goal::new(user_id, "  ".to_string(), target, None, None).is_err());

        // Alvo zero deve falhar
        let zero = Money::ZERO;
        assert!(Goal::new(user_id, "Inválido".to_string(), zero, None, None).is_err());
    }

    #[test]
    fn test_goal_progress_calculation() {
        let user_id = UserId::generate();
        let target = Money::from_decimal_non_negative(dec!(10000.00)).unwrap();
        let target_date = NaiveDate::from_ymd_opt(2027, 4, 15).unwrap();
        let goal = Goal::new(
            user_id,
            "Reserva".to_string(),
            target,
            Some(target_date),
            None,
        )
        .unwrap();

        let current = Money::from_decimal_non_negative(dec!(4000.00)).unwrap();
        let rate = Money::from_decimal_non_negative(dec!(1000.00)).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();

        let progress = GoalProgress::calculate(goal, current, Some(rate), today, vec![]);

        assert_eq!(progress.percentage, dec!(40.00));
        assert_eq!(progress.remaining_amount.as_decimal(), dec!(6000.00));
        assert!(!progress.is_completed);

        // 6 meses restantes (Outubro/2026 a Abril/2027) -> 6000 / 6 = 1000.00
        assert_eq!(progress.monthly_needed.unwrap().as_decimal(), dec!(1000.00));

        // Com taxa de 1000/mês e falta 6000 -> 6 meses a partir de Outubro/2026 -> Abril/2027
        assert_eq!(
            progress.estimated_completion_date,
            Some(NaiveDate::from_ymd_opt(2027, 4, 15).unwrap())
        );
    }

    #[test]
    fn test_goal_completed_when_target_reached() {
        let user_id = UserId::generate();
        let target = Money::from_decimal_non_negative(dec!(1000.00)).unwrap();
        let goal = Goal::new(user_id, "Viagem".to_string(), target, None, None).unwrap();

        let current = Money::from_decimal_non_negative(dec!(1200.00)).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();

        let progress = GoalProgress::calculate(goal, current, None, today, vec![]);
        assert!(progress.is_completed);
        assert_eq!(progress.percentage, dec!(120.00));
        assert_eq!(progress.remaining_amount.as_decimal(), dec!(0.00));
        assert_eq!(progress.monthly_needed, None);
        assert_eq!(progress.estimated_completion_date, None);
    }

    #[test]
    fn test_add_months_eom_rule_d05() {
        // 31 de janeiro + 1 mês em ano não bissexto -> 28 de fevereiro
        let jan31 = NaiveDate::from_ymd_opt(2027, 1, 31).unwrap();
        let feb28 = add_months(jan31, 1).unwrap();
        assert_eq!(feb28, NaiveDate::from_ymd_opt(2027, 2, 28).unwrap());

        // 31 de março + 1 mês -> 30 de abril
        let mar31 = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
        let apr30 = add_months(mar31, 1).unwrap();
        assert_eq!(apr30, NaiveDate::from_ymd_opt(2026, 4, 30).unwrap());
    }
}
