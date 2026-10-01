use crate::errors::AppError;
use crate::services::import_service::{find_column_index, load_profile, parse_amount, parse_date};
use chrono::NaiveDate;
use domain::{Money, TransactionId, TransactionKind, UserId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use storage::{AccountRepository, TransactionDetails, TransactionFilter, TransactionRepository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtratoRow {
    pub line_number: usize,
    pub date: NaiveDate,
    pub amount: Money,
    pub kind: TransactionKind,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileMatchPair {
    pub extrato_row: ExtratoRow,
    pub transaction: TransactionDetails,
    pub days_difference: i64,
    pub similarity_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileAnalysis {
    pub account_name: String,
    pub matched_pairs: Vec<ReconcileMatchPair>,
    pub unmatched_csv_rows: Vec<ExtratoRow>,
    pub unmatched_db_transactions: Vec<TransactionDetails>,
    pub applied: bool,
    pub reconciled_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileStatusSummary {
    pub account_name: Option<String>,
    pub total_unreconciled_count: usize,
    pub total_unreconciled_amount: Money,
    pub transactions: Vec<TransactionDetails>,
}

#[derive(Debug, Clone)]
pub struct ReconcileInput {
    pub user_id: UserId,
    pub account_query: String,
    pub file_path: String,
    pub profile: Option<String>,
    pub max_days: Option<i64>,
    pub apply: bool,
}

pub struct ReconcileService<'a> {
    pool: &'a PgPool,
}

impl<'a> ReconcileService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn reconcile(&self, input: ReconcileInput) -> Result<ReconcileAnalysis, AppError> {
        let account =
            AccountRepository::find_by_id_or_name(self.pool, input.user_id, &input.account_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Conta '{}' não encontrada.", input.account_query))
                })?;

        let max_days = input.max_days.unwrap_or(3);
        let profile = load_profile(input.profile.as_deref())?;

        // Read and parse CSV
        let path = Path::new(&input.file_path);
        if !path.exists() {
            return Err(AppError::Validation(format!(
                "Arquivo '{}' não encontrado.",
                input.file_path
            )));
        }

        let mut file = File::open(path).map_err(|e| {
            AppError::Validation(format!("Erro ao abrir arquivo '{}': {e}", input.file_path))
        })?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| AppError::Validation(format!("Erro ao ler arquivo: {e}")))?;

        // Strip UTF-8 BOM if present
        let content_bytes = if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
            &buffer[3..]
        } else {
            &buffer[..]
        };

        let delimiter = profile.delimiter.unwrap_or_else(|| {
            let sample = String::from_utf8_lossy(&content_bytes[..content_bytes.len().min(1024)]);
            let comma_count = sample.matches(',').count();
            let semi_count = sample.matches(';').count();
            let tab_count = sample.matches('\t').count();
            if semi_count > comma_count && semi_count > tab_count {
                ';'
            } else if tab_count > comma_count && tab_count > semi_count {
                '\t'
            } else {
                ','
            }
        });

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter as u8)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(content_bytes);

        let headers = rdr
            .headers()
            .map_err(|e| AppError::Validation(format!("Erro ao ler cabeçalho do CSV: {e}")))?
            .clone();

        let date_idx = find_column_index(
            &headers,
            &profile.date_column,
            &["data", "date", "dt", "data lançamento", "data_lancamento"],
        )
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Coluna de data '{}' não encontrada no CSV.",
                profile.date_column
            ))
        })?;

        let desc_idx = find_column_index(
            &headers,
            &profile.description_column,
            &[
                "descricao",
                "descrição",
                "description",
                "memo",
                "title",
                "historico",
                "histórico",
            ],
        )
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Coluna de descrição '{}' não encontrada no CSV.",
                profile.description_column
            ))
        })?;

        let amount_idx = find_column_index(
            &headers,
            &profile.amount_column,
            &["valor", "amount", "value"],
        )
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Coluna de valor '{}' não encontrada no CSV.",
                profile.amount_column
            ))
        })?;

        let kind_idx = profile
            .kind_column
            .as_ref()
            .and_then(|k| find_column_index(&headers, k, &["tipo", "kind", "type"]));

        let mut extrato_rows = Vec::new();

        for (idx, result) in rdr.records().enumerate() {
            let line_number = idx + 2;
            let record = match result {
                Ok(r) => r,
                Err(_) => continue,
            };

            let date_raw = record.get(date_idx).unwrap_or("").trim();
            let desc_raw = record.get(desc_idx).unwrap_or("").trim();
            let amount_raw = record.get(amount_idx).unwrap_or("").trim();

            if date_raw.is_empty() && desc_raw.is_empty() && amount_raw.is_empty() {
                continue;
            }

            let date = match parse_date(date_raw) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let (parsed_amount_dec, is_negative_amount) = match parse_amount(amount_raw) {
                Ok((dec, is_neg)) => (dec, is_neg),
                Err(_) => continue,
            };

            let kind = if let Some(k_idx) = kind_idx {
                let kind_raw = record.get(k_idx).unwrap_or("").trim();
                if let Ok(k) = kind_raw.parse::<TransactionKind>() {
                    k
                } else if is_negative_amount {
                    TransactionKind::Expense
                } else {
                    TransactionKind::Income
                }
            } else if is_negative_amount {
                TransactionKind::Expense
            } else {
                TransactionKind::Income
            };

            let money = match Money::from_decimal_non_negative(parsed_amount_dec) {
                Ok(m) => m,
                Err(_) => continue,
            };

            extrato_rows.push(ExtratoRow {
                line_number,
                date,
                amount: money,
                kind,
                description: desc_raw.to_string(),
            });
        }

        // Fetch all unreconciled transactions for this account
        let mut unreconciled_db_txs = TransactionRepository::list_with_details(
            self.pool,
            TransactionFilter {
                user_id: input.user_id,
                account_id: Some(account.id),
                reconciled: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let mut matched_pairs = Vec::new();
        let mut unmatched_csv_rows = Vec::new();
        let mut matched_tx_ids = Vec::new();

        for row in extrato_rows {
            // Find candidates in unreconciled_db_txs
            let mut best_match_idx: Option<usize> = None;
            let mut best_score: f32 = -1.0;
            let mut best_days_diff: i64 = 0;

            for (idx, tx) in unreconciled_db_txs.iter().enumerate() {
                if tx.kind != row.kind || tx.amount != row.amount {
                    continue;
                }

                let days_diff = (tx.date - row.date).num_days().abs();
                if days_diff > max_days {
                    continue;
                }

                let sim = string_similarity(&row.description, &tx.description);
                let date_score = 1.0 - (days_diff as f32 / (max_days as f32 + 1.0));
                let score = date_score * 0.4 + sim * 0.6;

                if score > best_score {
                    best_score = score;
                    best_match_idx = Some(idx);
                    best_days_diff = days_diff;
                }
            }

            if let Some(idx) = best_match_idx {
                let matched_tx = unreconciled_db_txs.remove(idx);
                matched_tx_ids.push(matched_tx.id);
                matched_pairs.push(ReconcileMatchPair {
                    extrato_row: row,
                    transaction: matched_tx,
                    days_difference: best_days_diff,
                    similarity_score: best_score,
                });
            } else {
                unmatched_csv_rows.push(row);
            }
        }

        let reconciled_count = if input.apply && !matched_tx_ids.is_empty() {
            TransactionRepository::mark_reconciled(self.pool, input.user_id, &matched_tx_ids)
                .await? as usize
        } else {
            0
        };

        Ok(ReconcileAnalysis {
            account_name: account.name,
            matched_pairs,
            unmatched_csv_rows,
            unmatched_db_transactions: unreconciled_db_txs,
            applied: input.apply,
            reconciled_count,
        })
    }

    pub async fn apply_reconciliation(
        &self,
        user_id: UserId,
        matched_tx_ids: &[TransactionId],
    ) -> Result<u64, AppError> {
        let count =
            TransactionRepository::mark_reconciled(self.pool, user_id, matched_tx_ids).await?;
        Ok(count)
    }

    pub async fn status(
        &self,
        user_id: UserId,
        account_query: Option<String>,
    ) -> Result<ReconcileStatusSummary, AppError> {
        let account = if let Some(ref acc_q) = account_query {
            let acc = AccountRepository::find_by_id_or_name(self.pool, user_id, acc_q)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Conta '{acc_q}' não encontrada.")))?;
            Some(acc)
        } else {
            None
        };

        let account_id = account.as_ref().map(|a| a.id);
        let account_name = account.map(|a| a.name);

        let transactions = TransactionRepository::list_with_details(
            self.pool,
            TransactionFilter {
                user_id,
                account_id,
                reconciled: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let total_dec: Decimal = transactions.iter().map(|t| t.amount.as_decimal()).sum();
        let total_money = Money::from_decimal_non_negative(total_dec)
            .unwrap_or_else(|_| Money::new(Decimal::ZERO).unwrap());

        Ok(ReconcileStatusSummary {
            account_name,
            total_unreconciled_count: transactions.len(),
            total_unreconciled_amount: total_money,
            transactions,
        })
    }
}

fn string_similarity(s1: &str, s2: &str) -> f32 {
    let tokens1: HashSet<String> = s1
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();

    let tokens2: HashSet<String> = s2
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();

    if tokens1.is_empty() && tokens2.is_empty() {
        return 1.0;
    }
    if tokens1.is_empty() || tokens2.is_empty() {
        return 0.0;
    }

    let intersection_count = tokens1.intersection(&tokens2).count();
    (2.0 * intersection_count as f32) / (tokens1.len() + tokens2.len()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similarity() {
        assert!(string_similarity("Uber *Trip 123", "Uber Trip") > 0.6);
        assert_eq!(string_similarity("PADARIA", "FARMACIA"), 0.0);
    }
}
