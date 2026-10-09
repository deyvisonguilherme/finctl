use crate::errors::AppError;
use chrono::{NaiveDate, Utc};
use domain::{
    add_months, calculate_invoice_dates_for_transaction, generate_forecast_intervals,
    months_difference, Account, AccountId, AccountKind, CashflowForecast, ForecastGranularity,
    ForecastPeriod, InvoiceStatus, RecurringRule, TransactionKind, TransactionStatus, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use storage::{
    AccountRepository, CardInvoiceRepository, GoalRepository, RecurringRepository,
    TransactionFilter, TransactionRepository,
};

pub struct ForecastInput {
    pub user_id: UserId,
    pub months: Option<u32>,
    pub account_query: Option<String>,
    pub granularity: Option<ForecastGranularity>,
    pub as_of_date: Option<NaiveDate>,
    pub include_goals: bool,
}

#[derive(Debug, Clone)]
struct FlowEvent {
    pub date: NaiveDate,
    pub amount: Decimal, // Positivo para receita/entrada, negativo para despesa/saída
    pub account_id: AccountId,
    pub is_transfer: bool,
    #[allow(dead_code)]
    pub description: String,
}

pub struct ForecastService<'a> {
    pool: &'a PgPool,
}

impl<'a> ForecastService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn generate_forecast(
        &self,
        input: ForecastInput,
    ) -> Result<CashflowForecast, AppError> {
        let user_id = input.user_id;
        let today = input.as_of_date.unwrap_or_else(|| Utc::now().date_naive());
        let months = input.months.unwrap_or(3).max(1);
        let granularity = input.granularity.unwrap_or(ForecastGranularity::Month);

        // Horizonte final
        let horizon_end = add_months(today, months)
            .unwrap_or_else(|| today + chrono::Duration::days(months as i64 * 31));

        // 1. Carregar contas do usuário
        let all_accounts = AccountRepository::list_by_user(self.pool, user_id).await?;
        let accounts_map: HashMap<AccountId, Account> =
            all_accounts.iter().map(|a| (a.id, a.clone())).collect();

        // 2. Filtrar conta específica se solicitada
        let selected_account = if let Some(ref acc_q) = input.account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc)
        } else {
            None
        };

        // 3. Saldo inicial base realizado (status = 'paid')
        let initial_balance = if let Some(ref acc) = selected_account {
            GoalRepository::get_account_balance(self.pool, user_id, acc.id).await?
        } else {
            let mut sum_balance = Decimal::ZERO;
            for acc in &all_accounts {
                if acc.kind != AccountKind::CreditCard {
                    let b = GoalRepository::get_account_balance(self.pool, user_id, acc.id).await?;
                    sum_balance += b;
                }
            }
            sum_balance
        };

        // 4. Carregar faturas pagas de cartões para evitar duplicidades
        let mut paid_invoice_months: HashMap<AccountId, HashSet<String>> = HashMap::new();
        for acc in &all_accounts {
            if acc.kind == AccountKind::CreditCard {
                let invoices =
                    CardInvoiceRepository::list_by_account(self.pool, user_id, acc.id).await?;
                let paid_set: HashSet<String> = invoices
                    .into_iter()
                    .filter(|i| i.status == InvoiceStatus::Paid)
                    .map(|i| i.month)
                    .collect();
                paid_invoice_months.insert(acc.id, paid_set);
            }
        }

        // 5. Coleta de eventos projetados
        let mut events: Vec<FlowEvent> = Vec::new();

        // 5.1. Transações pendentes (inclui parcelas futuras e lançamentos agendados)
        let pending_filter = TransactionFilter {
            user_id,
            status: Some(TransactionStatus::Pending),
            deleted: Some(false),
            ..Default::default()
        };
        let pending_txs =
            TransactionRepository::list_with_details(self.pool, pending_filter).await?;

        for tx in pending_txs {
            let acc = match accounts_map.get(&tx.account_id) {
                Some(a) => a,
                None => continue,
            };

            let is_transfer = tx.transfer_id.is_some();

            if acc.kind == AccountKind::CreditCard {
                // Compra em cartão de crédito: regime de caixa (D-08)
                // O débito ocorre na data de vencimento da fatura
                let closing_day = acc.closing_day.unwrap_or(1);
                let due_day = acc.due_day.unwrap_or(10);
                let (_, _, due_date) =
                    calculate_invoice_dates_for_transaction(closing_day, due_day, tx.date);

                let event_date = if due_date <= today { today } else { due_date };
                if event_date <= horizon_end {
                    let amount_dec = match tx.kind {
                        TransactionKind::Expense => -tx.amount.as_decimal(),
                        TransactionKind::Income => tx.amount.as_decimal(),
                    };
                    events.push(FlowEvent {
                        date: event_date,
                        amount: amount_dec,
                        account_id: tx.account_id,
                        is_transfer,
                        description: tx.description,
                    });
                }
            } else {
                // Conta bancária líquida
                let event_date = if tx.date <= today { today } else { tx.date };
                if event_date <= horizon_end {
                    let amount_dec = match tx.kind {
                        TransactionKind::Income => tx.amount.as_decimal(),
                        TransactionKind::Expense => -tx.amount.as_decimal(),
                    };
                    events.push(FlowEvent {
                        date: event_date,
                        amount: amount_dec,
                        account_id: tx.account_id,
                        is_transfer,
                        description: tx.description,
                    });
                }
            }
        }

        // 5.2. Compras em cartão com status 'paid' cuja fatura ainda NÃO venceu (D-08)
        // No finctl, compras em cartão são cadastradas como 'paid' na data da compra,
        // mas a saída de caixa só ocorre no vencimento da fatura!
        let card_tx_filter = TransactionFilter {
            user_id,
            status: Some(TransactionStatus::Paid),
            deleted: Some(false),
            ..Default::default()
        };
        let card_txs = TransactionRepository::list_with_details(self.pool, card_tx_filter).await?;

        for tx in card_txs {
            let acc = match accounts_map.get(&tx.account_id) {
                Some(a) => a,
                None => continue,
            };

            if acc.kind != AccountKind::CreditCard {
                continue;
            }

            // Ignorar transferências de pagamento de fatura recebidas no cartão
            if tx.transfer_id.is_some() {
                continue;
            }

            let closing_day = acc.closing_day.unwrap_or(1);
            let due_day = acc.due_day.unwrap_or(10);
            let (ref_month, _, due_date) =
                calculate_invoice_dates_for_transaction(closing_day, due_day, tx.date);

            // Se a fatura desse mês já foi paga, a saída bancária já ocorreu (não duplicar)
            if let Some(paid_months) = paid_invoice_months.get(&acc.id) {
                if paid_months.contains(&ref_month) {
                    continue;
                }
            }

            // Se o vencimento é futuro (>= today) ou hoje, a saída ainda vai ocorrer
            if due_date >= today && due_date <= horizon_end {
                let amount_dec = match tx.kind {
                    TransactionKind::Expense => -tx.amount.as_decimal(),
                    TransactionKind::Income => tx.amount.as_decimal(),
                };
                events.push(FlowEvent {
                    date: due_date,
                    amount: amount_dec,
                    account_id: tx.account_id,
                    is_transfer: false,
                    description: format!("Fatura {}: {}", ref_month, tx.description),
                });
            }
        }

        // 5.3. Ocorrências futuras de regras recorrentes (apenas após last_generated_date)
        let recurring_rules =
            RecurringRepository::list_by_user(self.pool, user_id, Some(true)).await?;

        for rule_detail in recurring_rules {
            let acc = match accounts_map.get(&rule_detail.account_id) {
                Some(a) => a,
                None => continue,
            };

            let rule = RecurringRule {
                id: rule_detail.id,
                user_id: rule_detail.user_id,
                account_id: rule_detail.account_id,
                category_id: rule_detail.category_id,
                kind: rule_detail.kind,
                amount: rule_detail.amount,
                description: rule_detail.description.clone(),
                frequency: rule_detail.frequency,
                day_of_month: rule_detail.day_of_month,
                day_of_week: rule_detail.day_of_week,
                start_date: rule_detail.start_date,
                end_date: rule_detail.end_date,
                active: rule_detail.active,
                last_generated_date: rule_detail.last_generated_date,
                created_at: rule_detail.created_at,
                updated_at: rule_detail.updated_at,
            };

            // calculate_occurrences_until já filtra apenas ocorrências onde curr > last_generated_date!
            let occurrences = rule.calculate_occurrences_until(horizon_end);

            for occ_date in occurrences {
                // Apenas ocorrências a partir de hoje
                if occ_date < today {
                    continue;
                }

                if acc.kind == AccountKind::CreditCard {
                    // Recorrência em cartão de crédito: cai no vencimento da fatura correspondente
                    let closing_day = acc.closing_day.unwrap_or(1);
                    let due_day = acc.due_day.unwrap_or(10);
                    let (_, _, due_date) =
                        calculate_invoice_dates_for_transaction(closing_day, due_day, occ_date);

                    let event_date = if due_date <= today { today } else { due_date };
                    if event_date <= horizon_end {
                        let amount_dec = match rule.kind {
                            TransactionKind::Expense => -rule.amount.as_decimal(),
                            TransactionKind::Income => rule.amount.as_decimal(),
                        };
                        events.push(FlowEvent {
                            date: event_date,
                            amount: amount_dec,
                            account_id: rule.account_id,
                            is_transfer: false,
                            description: format!("Recorrente: {}", rule.description),
                        });
                    }
                } else {
                    let amount_dec = match rule.kind {
                        TransactionKind::Income => rule.amount.as_decimal(),
                        TransactionKind::Expense => -rule.amount.as_decimal(),
                    };
                    events.push(FlowEvent {
                        date: occ_date,
                        amount: amount_dec,
                        account_id: rule.account_id,
                        is_transfer: false,
                        description: format!("Recorrente: {}", rule.description),
                    });
                }
            }
        }

        // 5.4. Aportes planejados de metas ativas (se include_goals for true)
        if input.include_goals {
            let active_goals = GoalRepository::list_by_user(self.pool, user_id, false).await?;
            for goal in active_goals {
                // Metas com conta bancária vinculada já usam o saldo bancário da conta (evitar duplicidade)
                if goal.is_account_linked() {
                    continue;
                }

                if let Some(target_date) = goal.target_date {
                    if target_date > today {
                        let current_sum =
                            GoalRepository::get_manual_goal_sum(self.pool, user_id, goal.id)
                                .await?;
                        let target_dec = goal.target_amount.as_decimal();
                        if current_sum < target_dec {
                            let remaining = target_dec - current_sum;
                            let months_left = months_difference(today, target_date).max(1);
                            let monthly_needed =
                                (remaining / Decimal::from(months_left)).round_dp(2);

                            let mut m = 0;
                            while m < months_left {
                                if let Some(plan_date) = add_months(today, m) {
                                    if plan_date > target_date || plan_date > horizon_end {
                                        break;
                                    }
                                    events.push(FlowEvent {
                                        date: plan_date,
                                        amount: -monthly_needed,
                                        account_id: AccountId::default(),
                                        is_transfer: false,
                                        description: format!("Aporte Meta: {}", goal.name),
                                    });
                                }
                                m += 1;
                            }
                        }
                    }
                }
            }
        }

        // 6. Geração dos intervalos e agregação
        let intervals = generate_forecast_intervals(today, horizon_end, granularity);

        let mut current_opening = initial_balance;
        let mut periods = Vec::with_capacity(intervals.len());

        for (idx, (label, start_date, end_date)) in intervals.into_iter().enumerate() {
            let mut total_income = Decimal::ZERO;
            let mut total_expense = Decimal::ZERO;

            for event in &events {
                // Filtro de conta se especificada
                if let Some(ref sel_acc) = selected_account {
                    if event.account_id != sel_acc.id {
                        continue;
                    }
                } else {
                    // Visão consolidada: transferências entre contas bancárias próprias se anulam
                    if event.is_transfer {
                        continue;
                    }
                }

                // Checagem de intervalo
                let in_period = if idx == 0 {
                    // Primeiro período: absorve eventos de data <= end_date (incluindo pendentes de hoje ou passados)
                    event.date <= end_date
                } else {
                    event.date >= start_date && event.date <= end_date
                };

                if in_period {
                    if event.amount > Decimal::ZERO {
                        total_income += event.amount;
                    } else {
                        total_expense += -event.amount;
                    }
                }
            }

            let net_change = total_income - total_expense;
            let closing_balance = current_opening + net_change;
            let is_negative = closing_balance < Decimal::ZERO;

            periods.push(ForecastPeriod {
                period_label: label,
                start_date,
                end_date,
                opening_balance: current_opening,
                total_income,
                total_expense,
                net_change,
                closing_balance,
                is_negative,
            });

            current_opening = closing_balance;
        }

        let forecast = CashflowForecast::new(
            today,
            granularity,
            months,
            selected_account.as_ref().map(|a| a.id),
            selected_account.as_ref().map(|a| a.name.clone()),
            initial_balance,
            periods,
        );

        Ok(forecast)
    }
}
