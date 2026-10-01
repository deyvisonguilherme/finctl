use crate::errors::StorageError;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    calculate_invoice_dates_for_month, calculate_invoice_dates_for_transaction,
    next_reference_month, Account, AccountId, CardInvoice, CardInvoiceId, InvoiceStatus, UserId,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct CardInvoiceRepository;

impl CardInvoiceRepository {
    pub async fn create(pool: &PgPool, invoice: &CardInvoice) -> Result<(), StorageError> {
        let res = sqlx::query(
            r#"
            INSERT INTO card_invoices (id, user_id, account_id, month, closing_date, due_date, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(invoice.id.as_uuid())
        .bind(invoice.user_id.as_uuid())
        .bind(invoice.account_id.as_uuid())
        .bind(&invoice.month)
        .bind(invoice.closing_date)
        .bind(invoice.due_date)
        .bind(invoice.status.as_str())
        .bind(invoice.created_at)
        .bind(invoice.updated_at)
        .execute(pool)
        .await;

        match res {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::UniqueViolation(format!(
                    "Já existe uma fatura para o mês '{}' nesta conta.",
                    invoice.month
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    pub async fn find_by_account_and_month(
        pool: &PgPool,
        account_id: AccountId,
        month: &str,
    ) -> Result<Option<CardInvoice>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, account_id, month, closing_date, due_date, status, created_at, updated_at
            FROM card_invoices
            WHERE account_id = $1 AND month = $2
            LIMIT 1
            "#,
        )
        .bind(account_id.as_uuid())
        .bind(month.trim())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(r) = row {
            Ok(Some(map_invoice_row(r)?))
        } else {
            Ok(None)
        }
    }

    pub async fn find_by_id(
        pool: &PgPool,
        user_id: UserId,
        id: CardInvoiceId,
    ) -> Result<Option<CardInvoice>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, account_id, month, closing_date, due_date, status, created_at, updated_at
            FROM card_invoices
            WHERE user_id = $1 AND id = $2
            LIMIT 1
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(StorageError::Database)?;

        if let Some(r) = row {
            Ok(Some(map_invoice_row(r)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_by_account(
        pool: &PgPool,
        user_id: UserId,
        account_id: AccountId,
    ) -> Result<Vec<CardInvoice>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, account_id, month, closing_date, due_date, status, created_at, updated_at
            FROM card_invoices
            WHERE user_id = $1 AND account_id = $2
            ORDER BY month ASC
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(account_id.as_uuid())
        .fetch_all(pool)
        .await
        .map_err(StorageError::Database)?;

        let mut invoices = Vec::with_capacity(rows.len());
        for row in rows {
            invoices.push(map_invoice_row(row)?);
        }
        Ok(invoices)
    }

    pub async fn update_status(
        pool: &PgPool,
        user_id: UserId,
        id: CardInvoiceId,
        status: InvoiceStatus,
    ) -> Result<(), StorageError> {
        let rows_affected = sqlx::query(
            r#"
            UPDATE card_invoices
            SET status = $1, updated_at = $2
            WHERE user_id = $3 AND id = $4
            "#,
        )
        .bind(status.as_str())
        .bind(Utc::now())
        .bind(user_id.as_uuid())
        .bind(id.as_uuid())
        .execute(pool)
        .await
        .map_err(StorageError::Database)?
        .rows_affected();

        if rows_affected == 0 {
            return Err(StorageError::NotFound(format!(
                "Fatura com ID '{id}' não encontrada."
            )));
        }
        Ok(())
    }

    /// Resolve ou cria a fatura para uma compra em `tx_date`.
    /// Se a fatura candidata estiver fechada ou paga, avança para o próximo mês de referência até encontrar/criar uma fatura aberta.
    pub async fn get_or_create_for_transaction(
        pool: &PgPool,
        user_id: UserId,
        account: &Account,
        tx_date: NaiveDate,
    ) -> Result<CardInvoice, StorageError> {
        let closing_day = account.closing_day.unwrap_or(20);
        let due_day = account.due_day.unwrap_or(27);

        let (mut cur_month, mut closing_date, mut due_date) =
            calculate_invoice_dates_for_transaction(closing_day, due_day, tx_date);

        // Se a fatura encontrada já estiver 'closed' ou 'paid', deve avançar para o próximo mês
        loop {
            let existing = Self::find_by_account_and_month(pool, account.id, &cur_month).await?;
            match existing {
                Some(inv) => {
                    if inv.status == InvoiceStatus::Open {
                        return Ok(inv);
                    }
                    // Se estiver fechada ou paga, calcula o próximo mês
                    let next_m = next_reference_month(&cur_month)
                        .map_err(|e| StorageError::Conversion(e.to_string()))?;
                    let parts: Vec<&str> = next_m.split('-').collect();
                    let y: i32 = parts[0].parse().unwrap();
                    let m: u32 = parts[1].parse().unwrap();
                    let (cd, dd) = calculate_invoice_dates_for_month(closing_day, due_day, y, m);
                    cur_month = next_m;
                    closing_date = cd;
                    due_date = dd;
                }
                None => {
                    let new_inv = CardInvoice::new(
                        user_id,
                        account.id,
                        cur_month.clone(),
                        closing_date,
                        due_date,
                    )
                    .map_err(|e| StorageError::Conversion(e.to_string()))?;

                    match Self::create(pool, &new_inv).await {
                        Ok(_) => return Ok(new_inv),
                        Err(StorageError::UniqueViolation(_)) => {
                            // Concorrência: busca existente
                            if let Some(inv) =
                                Self::find_by_account_and_month(pool, account.id, &cur_month)
                                    .await?
                            {
                                if inv.status == InvoiceStatus::Open {
                                    return Ok(inv);
                                }
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }
}

fn map_invoice_row(row: sqlx::postgres::PgRow) -> Result<CardInvoice, StorageError> {
    let id: Uuid = row.try_get("id").map_err(StorageError::Database)?;
    let user_id: Uuid = row.try_get("user_id").map_err(StorageError::Database)?;
    let account_id: Uuid = row.try_get("account_id").map_err(StorageError::Database)?;
    let month: String = row.try_get("month").map_err(StorageError::Database)?;
    let closing_date: NaiveDate = row
        .try_get("closing_date")
        .map_err(StorageError::Database)?;
    let due_date: NaiveDate = row.try_get("due_date").map_err(StorageError::Database)?;
    let status_str: String = row.try_get("status").map_err(StorageError::Database)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(StorageError::Database)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(StorageError::Database)?;

    let status: InvoiceStatus = status_str
        .parse()
        .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

    Ok(CardInvoice {
        id: CardInvoiceId::new(id),
        user_id: UserId::new(user_id),
        account_id: AccountId::new(account_id),
        month,
        closing_date,
        due_date,
        status,
        created_at,
        updated_at,
    })
}
