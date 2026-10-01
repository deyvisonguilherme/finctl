use crate::errors::DomainError;
use crate::money::Money;
use crate::types::{
    AccountId, CategoryId, RecurringFrequency, RecurringRuleId, TransactionKind, UserId,
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringRule {
    pub id: RecurringRuleId,
    pub user_id: UserId,
    pub account_id: AccountId,
    pub category_id: CategoryId,
    pub kind: TransactionKind,
    pub amount: Money,
    pub description: String,
    pub frequency: RecurringFrequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub active: bool,
    pub last_generated_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RecurringRule {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: UserId,
        account_id: AccountId,
        category_id: CategoryId,
        kind: TransactionKind,
        amount: Money,
        description: String,
        frequency: RecurringFrequency,
        day_of_month: Option<u32>,
        day_of_week: Option<u32>,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    ) -> Result<Self, DomainError> {
        let trimmed_desc = description.trim().to_string();
        if trimmed_desc.is_empty() {
            return Err(DomainError::Validation(
                "A descrição da recorrência não pode estar vazia.".to_string(),
            ));
        }
        if trimmed_desc.len() > 255 {
            return Err(DomainError::Validation(
                "A descrição da recorrência não pode ter mais de 255 caracteres.".to_string(),
            ));
        }

        if let Some(end) = end_date {
            if end < start_date {
                return Err(DomainError::Validation(
                    "A data final da recorrência não pode ser anterior à data inicial.".to_string(),
                ));
            }
        }

        let (dom, dow) = match frequency {
            RecurringFrequency::Monthly => {
                let dom_val = day_of_month.unwrap_or_else(|| start_date.day());
                if !(1..=31).contains(&dom_val) {
                    return Err(DomainError::Validation(
                        "O dia do mês para recorrência mensal deve ser entre 1 e 31.".to_string(),
                    ));
                }
                (Some(dom_val), None)
            }
            RecurringFrequency::Weekly => {
                let dow_val =
                    day_of_week.unwrap_or_else(|| start_date.weekday().number_from_monday());
                if !(1..=7).contains(&dow_val) {
                    return Err(DomainError::Validation(
                        "O dia da semana para recorrência semanal deve ser entre 1 (Segunda) e 7 (Domingo).".to_string(),
                    ));
                }
                (None, Some(dow_val))
            }
            RecurringFrequency::Yearly => {
                let dom_val = day_of_month.unwrap_or_else(|| start_date.day());
                (Some(dom_val), None)
            }
        };

        let now = Utc::now();
        Ok(Self {
            id: RecurringRuleId::generate(),
            user_id,
            account_id,
            category_id,
            kind,
            amount,
            description: trimmed_desc,
            frequency,
            day_of_month: dom,
            day_of_week: dow,
            start_date,
            end_date,
            active: true,
            last_generated_date: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn pause(&mut self) {
        self.active = false;
        self.updated_at = Utc::now();
    }

    pub fn resume(&mut self) {
        self.active = true;
        self.updated_at = Utc::now();
    }

    pub fn calculate_occurrences_until(&self, until_date: NaiveDate) -> Vec<NaiveDate> {
        if !self.active {
            return Vec::new();
        }
        let max_end = match self.end_date {
            Some(end) => std::cmp::min(end, until_date),
            None => until_date,
        };
        if self.start_date > max_end {
            return Vec::new();
        }

        let mut dates = Vec::new();

        match self.frequency {
            RecurringFrequency::Weekly => {
                let target_dow = self
                    .day_of_week
                    .unwrap_or_else(|| self.start_date.weekday().number_from_monday());
                let start_dow = self.start_date.weekday().number_from_monday();
                let offset = (target_dow as i64 - start_dow as i64).rem_euclid(7);
                let mut curr = self.start_date + chrono::Duration::days(offset);

                while curr <= max_end {
                    if let Some(last_gen) = self.last_generated_date {
                        if curr > last_gen {
                            dates.push(curr);
                        }
                    } else {
                        dates.push(curr);
                    }
                    curr += chrono::Duration::days(7);
                }
            }
            RecurringFrequency::Monthly => {
                let target_dom = self.day_of_month.unwrap_or_else(|| self.start_date.day());
                let mut curr_year = self.start_date.year();
                let mut curr_month = self.start_date.month();

                let end_year = max_end.year();
                let end_month = max_end.month();

                while curr_year < end_year || (curr_year == end_year && curr_month <= end_month) {
                    let dim = days_in_month(curr_year, curr_month);
                    let day = target_dom.min(dim);
                    if let Some(candidate) = NaiveDate::from_ymd_opt(curr_year, curr_month, day) {
                        if candidate >= self.start_date && candidate <= max_end {
                            if let Some(last_gen) = self.last_generated_date {
                                if candidate > last_gen {
                                    dates.push(candidate);
                                }
                            } else {
                                dates.push(candidate);
                            }
                        }
                    }

                    if curr_month == 12 {
                        curr_year += 1;
                        curr_month = 1;
                    } else {
                        curr_month += 1;
                    }
                }
            }
            RecurringFrequency::Yearly => {
                let target_month = self.start_date.month();
                let target_dom = self.day_of_month.unwrap_or_else(|| self.start_date.day());
                let mut curr_year = self.start_date.year();
                let end_year = max_end.year();

                while curr_year <= end_year {
                    let dim = days_in_month(curr_year, target_month);
                    let day = target_dom.min(dim);
                    if let Some(candidate) = NaiveDate::from_ymd_opt(curr_year, target_month, day) {
                        if candidate >= self.start_date && candidate <= max_end {
                            if let Some(last_gen) = self.last_generated_date {
                                if candidate > last_gen {
                                    dates.push(candidate);
                                }
                            } else {
                                dates.push(candidate);
                            }
                        }
                    }
                    curr_year += 1;
                }
            }
        }

        dates
    }
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    if let Some(next_first) = next_month {
        next_first.pred_opt().map(|d| d.day()).unwrap_or(30)
    } else {
        30
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_create_monthly_recurring_rule() {
        let user_id = UserId::generate();
        let acc_id = AccountId::generate();
        let cat_id = CategoryId::generate();
        let amount = Money::new(dec!(2500.00)).unwrap();
        let start_date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();

        let mut rule = RecurringRule::new(
            user_id,
            acc_id,
            cat_id,
            TransactionKind::Expense,
            amount,
            "Aluguel do Apartamento".to_string(),
            RecurringFrequency::Monthly,
            Some(5),
            None,
            start_date,
            None,
        )
        .unwrap();

        assert_eq!(rule.frequency, RecurringFrequency::Monthly);
        assert_eq!(rule.day_of_month, Some(5));
        assert!(rule.active);

        rule.pause();
        assert!(!rule.active);

        rule.resume();
        assert!(rule.active);
    }

    #[test]
    fn test_calculate_occurrences_monthly_short_months_d05() {
        let user_id = UserId::generate();
        let acc_id = AccountId::generate();
        let cat_id = CategoryId::generate();
        let amount = Money::new(dec!(100.00)).unwrap();
        let start_date = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();

        let mut rule = RecurringRule::new(
            user_id,
            acc_id,
            cat_id,
            TransactionKind::Expense,
            amount,
            "Assinatura Mensal".to_string(),
            RecurringFrequency::Monthly,
            Some(31),
            None,
            start_date,
            None,
        )
        .unwrap();

        let until = NaiveDate::from_ymd_opt(2026, 4, 30).unwrap();
        let dates = rule.calculate_occurrences_until(until);

        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(), // Feb 28 (short month)
                NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(), // Apr 30 (short month)
            ]
        );

        // If last_generated_date is set to Feb 28, should only return Mar and Apr
        rule.last_generated_date = Some(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
        let remaining = rule.calculate_occurrences_until(until);
        assert_eq!(
            remaining,
            vec![
                NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            ]
        );
    }

    #[test]
    fn test_calculate_occurrences_weekly() {
        let user_id = UserId::generate();
        let acc_id = AccountId::generate();
        let cat_id = CategoryId::generate();
        let amount = Money::new(dec!(50.00)).unwrap();
        // 2026-10-01 is Thursday (4)
        let start_date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();

        let rule = RecurringRule::new(
            user_id,
            acc_id,
            cat_id,
            TransactionKind::Expense,
            amount,
            "Feira Semanal".to_string(),
            RecurringFrequency::Weekly,
            None,
            Some(1), // Monday
            start_date,
            None,
        )
        .unwrap();

        let until = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let dates = rule.calculate_occurrences_until(until);

        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
                NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
                NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
            ]
        );
    }
}
