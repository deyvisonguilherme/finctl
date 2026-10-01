use crate::errors::AppError;
use chrono::{Days, NaiveDate};
use domain::{
    calculate_invoice_dates_for_month, Account, AccountKind, CardInvoice, InvoiceStatus, Money,
    UserId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::{
    AccountRepository, CardInvoiceRepository, TransactionDetails, TransactionFilter,
    TransactionRepository,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInvoiceSummary {
    pub invoice: CardInvoice,
    pub total_amount: Money,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInvoiceDetails {
    pub account: Account,
    pub invoice: CardInvoice,
    pub transactions: Vec<TransactionDetails>,
    pub total_amount: Money,
    pub credit_limit: Option<Money>,
    pub available_limit: Option<Money>,
}

pub struct CardService<'a> {
    pool: &'a PgPool,
}

impl<'a> CardService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_card_account(
        &self,
        user_id: UserId,
        card_query: &str,
    ) -> Result<Account, AppError> {
        let account = AccountRepository::find_by_id_or_name(self.pool, user_id, card_query)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Cartão/Conta '{card_query}' não encontrada."))
            })?;

        if account.kind != AccountKind::CreditCard {
            return Err(AppError::Validation(format!(
                "A conta '{}' não é do tipo cartão de crédito (tipo: '{}').",
                account.name,
                account.kind.display_pt_br()
            )));
        }

        Ok(account)
    }

    pub fn get_invoice_date_range(
        closing_day: u8,
        due_day: u8,
        invoice_month: &str,
    ) -> Result<(NaiveDate, NaiveDate), AppError> {
        let parts: Vec<&str> = invoice_month.split('-').collect();
        if parts.len() != 2 {
            return Err(AppError::Validation(format!(
                "Mês de fatura inválido '{invoice_month}'."
            )));
        }

        let year: i32 = parts[0]
            .parse()
            .map_err(|_| AppError::Validation("Ano inválido.".to_string()))?;
        let month: u32 = parts[1]
            .parse()
            .map_err(|_| AppError::Validation("Mês inválido.".to_string()))?;

        let (cur_closing, _) = calculate_invoice_dates_for_month(closing_day, due_day, year, month);

        let (prev_year, prev_month) = if month == 1 {
            (year - 1, 12)
        } else {
            (year, month - 1)
        };

        let (prev_closing, _) =
            calculate_invoice_dates_for_month(closing_day, due_day, prev_year, prev_month);

        let start_date = prev_closing
            .checked_add_days(Days::new(1))
            .unwrap_or(prev_closing);
        let end_date = cur_closing;

        Ok((start_date, end_date))
    }

    pub async fn list_invoices(
        &self,
        user_id: UserId,
        card_query: &str,
    ) -> Result<Vec<CardInvoiceSummary>, AppError> {
        let card = self.get_card_account(user_id, card_query).await?;
        let invoices = CardInvoiceRepository::list_by_account(self.pool, user_id, card.id).await?;

        let closing_day = card.closing_day.unwrap_or(20);
        let due_day = card.due_day.unwrap_or(27);

        let mut summaries = Vec::with_capacity(invoices.len());

        for invoice in invoices {
            let (start_date, end_date) =
                Self::get_invoice_date_range(closing_day, due_day, &invoice.month)?;

            let txs = TransactionRepository::list_with_details(
                self.pool,
                TransactionFilter {
                    user_id,
                    account_id: Some(card.id),
                    from_date: Some(start_date),
                    to_date: Some(end_date),
                    ..Default::default()
                },
            )
            .await?;

            let total_dec: Decimal = txs
                .iter()
                .filter(|t| t.transfer_id.is_none())
                .map(|t| t.amount.as_decimal())
                .sum();
            let total_money = Money::from_decimal_non_negative(total_dec)
                .unwrap_or_else(|_| Money::new(Decimal::ZERO).unwrap());

            summaries.push(CardInvoiceSummary {
                item_count: txs.len(),
                total_amount: total_money,
                invoice,
            });
        }

        Ok(summaries)
    }

    pub async fn show_invoice(
        &self,
        user_id: UserId,
        card_query: &str,
        month: Option<String>,
    ) -> Result<CardInvoiceDetails, AppError> {
        let card = self.get_card_account(user_id, card_query).await?;
        let invoices = CardInvoiceRepository::list_by_account(self.pool, user_id, card.id).await?;

        if invoices.is_empty() {
            return Err(AppError::NotFound(format!(
                "Nenhuma fatura encontrada para o cartão '{}'.",
                card.name
            )));
        }

        let target_invoice = if let Some(ref m) = month {
            invoices
                .into_iter()
                .find(|inv| inv.month == m.trim())
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Fatura do mês '{m}' não encontrada para o cartão '{}'.",
                        card.name
                    ))
                })?
        } else {
            // Prioritize the oldest Open invoice, or the latest invoice if none open
            invoices
                .iter()
                .find(|inv| inv.status == InvoiceStatus::Open)
                .cloned()
                .unwrap_or_else(|| invoices.last().unwrap().clone())
        };

        let closing_day = card.closing_day.unwrap_or(20);
        let due_day = card.due_day.unwrap_or(27);
        let (start_date, end_date) =
            Self::get_invoice_date_range(closing_day, due_day, &target_invoice.month)?;

        let txs = TransactionRepository::list_with_details(
            self.pool,
            TransactionFilter {
                user_id,
                account_id: Some(card.id),
                from_date: Some(start_date),
                to_date: Some(end_date),
                ..Default::default()
            },
        )
        .await?;

        let total_dec: Decimal = txs
            .iter()
            .filter(|t| t.transfer_id.is_none())
            .map(|t| t.amount.as_decimal())
            .sum();
        let total_money = Money::from_decimal_non_negative(total_dec)
            .unwrap_or_else(|_| Money::new(Decimal::ZERO).unwrap());

        // Calculate available limit
        let available_limit = if let Some(limit) = card.credit_limit {
            // Sum of all unpaid/pending expenses on the card minus payments (transfers to card)
            let all_card_txs = TransactionRepository::list_with_details(
                self.pool,
                TransactionFilter {
                    user_id,
                    account_id: Some(card.id),
                    ..Default::default()
                },
            )
            .await?;

            let total_spent: Decimal = all_card_txs
                .iter()
                .filter(|t| t.kind == domain::TransactionKind::Expense && t.transfer_id.is_none())
                .map(|t| t.amount.as_decimal())
                .sum();

            let total_paid: Decimal = all_card_txs
                .iter()
                .filter(|t| t.kind == domain::TransactionKind::Income && t.transfer_id.is_some())
                .map(|t| t.amount.as_decimal())
                .sum();

            let used = (total_spent - total_paid).max(Decimal::ZERO);
            let avail = (limit.as_decimal() - used).max(Decimal::ZERO);
            Some(
                Money::from_decimal_non_negative(avail)
                    .unwrap_or_else(|_| Money::new(Decimal::ZERO).unwrap()),
            )
        } else {
            None
        };

        let credit_limit = card.credit_limit;
        Ok(CardInvoiceDetails {
            account: card,
            invoice: target_invoice,
            transactions: txs,
            total_amount: total_money,
            credit_limit,
            available_limit,
        })
    }

    pub async fn close_invoice(
        &self,
        user_id: UserId,
        card_query: &str,
        month: Option<String>,
    ) -> Result<CardInvoice, AppError> {
        let card = self.get_card_account(user_id, card_query).await?;
        let invoices = CardInvoiceRepository::list_by_account(self.pool, user_id, card.id).await?;

        if invoices.is_empty() {
            return Err(AppError::NotFound(format!(
                "Nenhuma fatura encontrada para o cartão '{}'.",
                card.name
            )));
        }

        let mut target_invoice = if let Some(ref m) = month {
            invoices
                .into_iter()
                .find(|inv| inv.month == m.trim())
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Fatura do mês '{m}' não encontrada para o cartão '{}'.",
                        card.name
                    ))
                })?
        } else {
            invoices
                .into_iter()
                .find(|inv| inv.status == InvoiceStatus::Open)
                .ok_or_else(|| {
                    AppError::Validation(format!(
                        "Não há faturas abertas para fechar no cartão '{}'.",
                        card.name
                    ))
                })?
        };

        if target_invoice.status == InvoiceStatus::Closed {
            return Err(AppError::Validation(format!(
                "A fatura de '{}' já está fechada.",
                target_invoice.month
            )));
        }

        if target_invoice.status == InvoiceStatus::Paid {
            return Err(AppError::Validation(format!(
                "A fatura de '{}' já está paga.",
                target_invoice.month
            )));
        }

        CardInvoiceRepository::update_status(
            self.pool,
            user_id,
            target_invoice.id,
            InvoiceStatus::Closed,
        )
        .await?;

        target_invoice.status = InvoiceStatus::Closed;
        Ok(target_invoice)
    }
}
