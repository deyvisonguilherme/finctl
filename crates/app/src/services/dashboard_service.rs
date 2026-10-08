use crate::errors::AppError;
use crate::services::{
    balance_service::BalanceReport, BalanceService, BudgetService, CardService,
    CategoryBudgetStatus, ReportService,
};
use chrono::{Datelike, Local, NaiveDate};
use domain::{AccountKind, InvoiceStatus, Money, TransactionKind, TransactionStatus, UserId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::{AccountRepository, TransactionFilter, TransactionRepository};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonthlySummary {
    pub month: String,
    pub total_income: Money,
    pub total_expense: Money,
    pub net_balance: Decimal,
    pub savings_rate: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpcomingKind {
    Expense,
    Income,
    CardInvoice,
}

impl UpcomingKind {
    pub fn display_pt_br(&self) -> &'static str {
        match self {
            UpcomingKind::Expense => "Despesa",
            UpcomingKind::Income => "Receita",
            UpcomingKind::CardInvoice => "Fatura Cartão",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpcomingDueItem {
    pub due_date: NaiveDate,
    pub description: String,
    pub amount: Money,
    pub kind: UpcomingKind,
    pub is_overdue: bool,
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardData {
    pub balance_report: BalanceReport,
    pub monthly_summary: MonthlySummary,
    pub budget_statuses: Vec<CategoryBudgetStatus>,
    pub upcoming_items: Vec<UpcomingDueItem>,
}

pub struct DashboardService<'a> {
    pool: &'a PgPool,
}

impl<'a> DashboardService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_dashboard_data(
        &self,
        user_id: UserId,
        target_month: Option<&str>,
    ) -> Result<DashboardData, AppError> {
        let month = match target_month {
            Some(m) => m.to_string(),
            None => {
                let today = Local::now().date_naive();
                format!("{:04}-{:02}", today.year(), today.month())
            }
        };

        // 1. Saldos por conta e total (idêntico a `finctl balance`)
        let balance_service = BalanceService::new(self.pool);
        let balance_report = balance_service.get_balance(user_id, None, false).await?;

        // 2. Resumo do mês (idêntico a `finctl report monthly`)
        let report_service = ReportService::new(self.pool);
        let monthly_reports = report_service
            .monthly_report(crate::MonthlyReportInput {
                user_id,
                month: Some(month.clone()),
                include_pending: false,
                ..Default::default()
            })
            .await?;

        let monthly_summary = if let Some(item) = monthly_reports.iter().find(|i| i.month == month)
        {
            MonthlySummary {
                month: item.month.clone(),
                total_income: item.total_income,
                total_expense: item.total_expense,
                net_balance: item.net_balance,
                savings_rate: item.savings_rate,
            }
        } else {
            MonthlySummary {
                month: month.clone(),
                total_income: Money::new(Decimal::ZERO)
                    .map_err(|e| AppError::Validation(e.to_string()))?,
                total_expense: Money::new(Decimal::ZERO)
                    .map_err(|e| AppError::Validation(e.to_string()))?,
                net_balance: Decimal::ZERO,
                savings_rate: Decimal::ZERO,
            }
        };

        // 3. Status dos orçamentos do mês (idêntico a `finctl budget status`)
        let budget_service = BudgetService::new(self.pool);
        let budget_statuses = budget_service
            .get_budget_status(user_id, month.clone())
            .await?;

        // 4. Próximos vencimentos (pendentes e faturas)
        let today = Local::now().date_naive();
        let mut upcoming_items = Vec::new();

        // 4.1. Lançamentos pendentes
        let pending_txs = TransactionRepository::list_with_details(
            self.pool,
            TransactionFilter {
                user_id,
                status: Some(TransactionStatus::Pending),
                deleted: Some(false),
                ..Default::default()
            },
        )
        .await?;

        for tx in pending_txs {
            if tx.transfer_id.is_some() {
                continue;
            }
            upcoming_items.push(UpcomingDueItem {
                due_date: tx.date,
                description: tx.description,
                amount: tx.amount,
                kind: match tx.kind {
                    TransactionKind::Expense => UpcomingKind::Expense,
                    TransactionKind::Income => UpcomingKind::Income,
                },
                is_overdue: tx.date < today,
                account_name: Some(tx.account_name),
            });
        }

        // 4.2. Faturas de cartão de crédito não pagas
        let accounts = AccountRepository::list_by_user(self.pool, user_id).await?;
        let card_service = CardService::new(self.pool);

        for acc in accounts {
            if acc.kind == AccountKind::CreditCard && !acc.is_deleted() {
                if let Ok(summaries) = card_service.list_invoices(user_id, &acc.name).await {
                    for s in summaries {
                        if s.invoice.status != InvoiceStatus::Paid {
                            let amount = if !s.remaining_amount.as_decimal().is_zero() {
                                s.remaining_amount
                            } else {
                                s.total_amount
                            };
                            if !amount.as_decimal().is_zero() || s.item_count > 0 {
                                upcoming_items.push(UpcomingDueItem {
                                    due_date: s.invoice.due_date,
                                    description: format!(
                                        "Fatura {} ({})",
                                        acc.name, s.invoice.month
                                    ),
                                    amount,
                                    kind: UpcomingKind::CardInvoice,
                                    is_overdue: s.invoice.due_date < today,
                                    account_name: Some(acc.name.clone()),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Ordenar cronologicamente por data de vencimento
        upcoming_items.sort_by_key(|item| item.due_date);
        upcoming_items.truncate(8);

        Ok(DashboardData {
            balance_report,
            monthly_summary,
            budget_statuses,
            upcoming_items,
        })
    }
}
