use crate::errors::AppError;
use chrono::NaiveDate;
use domain::{Category, Money, Transaction, TransactionKind, TransactionStatus, UserId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use storage::{AccountRepository, CategoryRepository, StorageError, TransactionRepository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvProfile {
    pub name: String,
    pub delimiter: Option<char>,
    pub date_column: String,
    pub description_column: String,
    pub amount_column: String,
    pub kind_column: Option<String>,
    pub category_column: Option<String>,
}

impl Default for CsvProfile {
    fn default() -> Self {
        Self {
            name: "generic".to_string(),
            delimiter: None,
            date_column: "data".to_string(),
            description_column: "descricao".to_string(),
            amount_column: "valor".to_string(),
            kind_column: Some("tipo".to_string()),
            category_column: Some("categoria".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowError {
    pub line_number: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub total_rows: usize,
    pub imported_count: usize,
    pub duplicates_count: usize,
    pub errors: Vec<ImportRowError>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct ImportCsvInput {
    pub user_id: UserId,
    pub account_query: String,
    pub file_path: String,
    pub profile_name: Option<String>,
    pub dry_run: bool,
}

pub struct ImportService<'a> {
    pool: &'a PgPool,
}

impl<'a> ImportService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn import_csv(&self, input: ImportCsvInput) -> Result<ImportSummary, AppError> {
        let account =
            AccountRepository::find_by_id_or_name(self.pool, input.user_id, &input.account_query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Conta '{}' não encontrada.", input.account_query))
                })?;

        let profile = load_profile(input.profile_name.as_deref())?;

        let content = fs::read_to_string(&input.file_path).map_err(|e| {
            AppError::Validation(format!("Erro ao ler arquivo '{}': {e}", input.file_path))
        })?;

        // Handle possible UTF-8 BOM
        let content_str = content.trim_start_matches('\u{feff}');

        // Determine delimiter
        let delimiter = profile
            .delimiter
            .unwrap_or_else(|| detect_delimiter(content_str));

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter as u8)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(content_str.as_bytes());

        let headers = rdr
            .headers()
            .map_err(|e| AppError::Validation(format!("Erro ao ler cabeçalho do CSV: {e}")))?
            .clone();

        let date_idx = find_column_index(&headers, &profile.date_column, &["data", "date", "dt"])
            .ok_or_else(|| {
            AppError::Validation(format!(
                "Coluna de data '{}' não encontrada no CSV.",
                profile.date_column
            ))
        })?;

        let desc_idx = find_column_index(
            &headers,
            &profile.description_column,
            &["descricao", "descrição", "description", "memo", "title"],
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

        let cat_idx = profile
            .category_column
            .as_ref()
            .and_then(|c| find_column_index(&headers, c, &["categoria", "category"]));

        // User's existing categories cache
        let categories = CategoryRepository::list_by_user(self.pool, input.user_id).await?;
        let mut category_map: HashMap<String, Category> = HashMap::new();
        for cat in categories {
            category_map.insert(cat.name.to_lowercase(), cat);
        }

        let mut total_rows = 0;
        let mut imported_count = 0;
        let mut duplicates_count = 0;
        let mut errors = Vec::new();
        let mut to_insert = Vec::new();

        // Track occurrences for identical (date, amount, kind, description) within the file
        let mut occurrence_counter: HashMap<String, u32> = HashMap::new();

        for (record_idx, result) in rdr.records().enumerate() {
            let line_number = record_idx + 2; // 1-indexed, header is line 1
            total_rows += 1;

            let record = match result {
                Ok(r) => r,
                Err(e) => {
                    errors.push(ImportRowError {
                        line_number,
                        reason: format!("Erro ao processar linha CSV: {e}"),
                    });
                    continue;
                }
            };

            let date_raw = record.get(date_idx).unwrap_or("").trim();
            let desc_raw = record.get(desc_idx).unwrap_or("").trim();
            let amount_raw = record.get(amount_idx).unwrap_or("").trim();

            if date_raw.is_empty() && desc_raw.is_empty() && amount_raw.is_empty() {
                // Ignore empty line
                total_rows -= 1;
                continue;
            }

            let date = match parse_date(date_raw) {
                Ok(d) => d,
                Err(e) => {
                    errors.push(ImportRowError {
                        line_number,
                        reason: format!("Data inválida '{date_raw}': {e}"),
                    });
                    continue;
                }
            };

            let (parsed_amount_dec, is_negative_amount) = match parse_amount(amount_raw) {
                Ok((dec, is_neg)) => (dec, is_neg),
                Err(e) => {
                    errors.push(ImportRowError {
                        line_number,
                        reason: format!("Valor monetário inválido '{amount_raw}': {e}"),
                    });
                    continue;
                }
            };

            let kind = if let Some(k_idx) = kind_idx {
                let kind_raw = record.get(k_idx).unwrap_or("").trim();
                if !kind_raw.is_empty() {
                    match kind_raw.parse::<TransactionKind>() {
                        Ok(k) => k,
                        Err(_) => {
                            if is_negative_amount {
                                TransactionKind::Expense
                            } else {
                                TransactionKind::Income
                            }
                        }
                    }
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

            let money = match Money::new(parsed_amount_dec) {
                Ok(m) => m,
                Err(e) => {
                    errors.push(ImportRowError {
                        line_number,
                        reason: format!("Valor monetário inválido: {e}"),
                    });
                    continue;
                }
            };

            let description = if desc_raw.is_empty() {
                "Lançamento importado".to_string()
            } else {
                desc_raw.to_string()
            };

            // Category resolution
            let category_raw = cat_idx.and_then(|idx| record.get(idx)).unwrap_or("").trim();

            let category = if !category_raw.is_empty() {
                if let Some(cat) = category_map.get(&category_raw.to_lowercase()) {
                    if cat.kind == kind {
                        cat.clone()
                    } else {
                        self.get_or_create_fallback_category(
                            input.user_id,
                            kind,
                            &mut category_map,
                            input.dry_run,
                        )
                        .await?
                    }
                } else {
                    self.get_or_create_fallback_category(
                        input.user_id,
                        kind,
                        &mut category_map,
                        input.dry_run,
                    )
                    .await?
                }
            } else {
                self.get_or_create_fallback_category(
                    input.user_id,
                    kind,
                    &mut category_map,
                    input.dry_run,
                )
                .await?
            };

            // Hash calculation
            let occurrence_key = format!(
                "{date}|{parsed_amount_dec:.2}|{}|{description}",
                kind.as_str()
            );
            let occurrence_index = occurrence_counter
                .entry(occurrence_key.clone())
                .or_insert(0);
            *occurrence_index += 1;

            let mut hasher = Sha256::new();
            hasher.update(format!("{occurrence_key}|{occurrence_index}").as_bytes());
            let hash_bytes = hasher.finalize();
            let import_hash = format!("{:x}", hash_bytes);

            // Check if duplicate in DB
            let duplicate_exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM transactions WHERE account_id = $1 AND import_hash = $2)",
            )
            .bind(account.id.as_uuid())
            .bind(&import_hash)
            .fetch_one(self.pool)
            .await
            .map_err(StorageError::Database)?;

            if duplicate_exists {
                duplicates_count += 1;
                continue;
            }

            if !input.dry_run {
                let tx = Transaction::new_full(
                    input.user_id,
                    account.id,
                    category.id,
                    kind,
                    money,
                    date,
                    description,
                    TransactionStatus::Paid,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(import_hash),
                    None,
                )?;

                to_insert.push(tx);
            }

            imported_count += 1;
        }

        if !input.dry_run && !to_insert.is_empty() {
            TransactionRepository::create_batch(self.pool, &to_insert).await?;
        }

        Ok(ImportSummary {
            total_rows,
            imported_count,
            duplicates_count,
            errors,
            dry_run: input.dry_run,
        })
    }

    async fn get_or_create_fallback_category(
        &self,
        user_id: UserId,
        kind: TransactionKind,
        category_map: &mut HashMap<String, Category>,
        dry_run: bool,
    ) -> Result<Category, AppError> {
        let fallback_name = match kind {
            TransactionKind::Expense => "A classificar",
            TransactionKind::Income => "A classificar (Receitas)",
        };

        if let Some(cat) = category_map.get(&fallback_name.to_lowercase()) {
            if cat.kind == kind {
                return Ok(cat.clone());
            }
        }

        // Try find in DB
        let existing =
            CategoryRepository::find_by_id_or_name(self.pool, user_id, fallback_name).await?;
        if let Some(cat) = existing {
            if cat.kind == kind {
                category_map.insert(fallback_name.to_lowercase(), cat.clone());
                return Ok(cat);
            }
        }

        let new_cat = Category::new_system(user_id, fallback_name.to_string(), kind)?;
        if !dry_run {
            CategoryRepository::create(self.pool, &new_cat).await?;
        }
        category_map.insert(fallback_name.to_lowercase(), new_cat.clone());
        Ok(new_cat)
    }
}

pub(crate) fn load_profile(name: Option<&str>) -> Result<CsvProfile, AppError> {
    let profile_name = name.unwrap_or("generic");
    if profile_name == "generic" {
        return Ok(CsvProfile::default());
    }

    if profile_name == "nubank" {
        return Ok(CsvProfile {
            name: "nubank".to_string(),
            delimiter: Some(','),
            date_column: "date".to_string(),
            description_column: "title".to_string(),
            amount_column: "amount".to_string(),
            kind_column: None,
            category_column: Some("category".to_string()),
        });
    }

    // Check custom profile in ~/.config/finctl/profiles/<profile_name>.toml
    if let Some(config_dir) = dirs_config_dir() {
        let path = config_dir
            .join("finctl")
            .join("profiles")
            .join(format!("{profile_name}.toml"));
        if path.exists() {
            let content = fs::read_to_string(&path).map_err(|e| {
                AppError::Validation(format!("Erro ao ler perfil '{}': {e}", path.display()))
            })?;
            let profile: CsvProfile = toml::from_str(&content).map_err(|e| {
                AppError::Validation(format!(
                    "Erro ao interpretar perfil TOML '{}': {e}",
                    path.display()
                ))
            })?;
            return Ok(profile);
        }
    }

    Err(AppError::NotFound(format!(
        "Perfil de importação '{profile_name}' não encontrado. Perfis disponíveis: 'generic', 'nubank'."
    )))
}

fn dirs_config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn detect_delimiter(sample: &str) -> char {
    let first_line = sample.lines().next().unwrap_or("");
    let comma_count = first_line.chars().filter(|c| *c == ',').count();
    let semicolon_count = first_line.chars().filter(|c| *c == ';').count();
    let tab_count = first_line.chars().filter(|c| *c == '\t').count();

    if semicolon_count > comma_count && semicolon_count >= tab_count {
        ';'
    } else if tab_count > comma_count && tab_count > semicolon_count {
        '\t'
    } else {
        ','
    }
}

pub(crate) fn find_column_index(
    headers: &csv::StringRecord,
    target: &str,
    aliases: &[&str],
) -> Option<usize> {
    let target_norm = target.trim().to_lowercase();
    for (i, h) in headers.iter().enumerate() {
        let h_norm = h.trim().to_lowercase();
        if h_norm == target_norm {
            return Some(i);
        }
    }

    for alias in aliases {
        for (i, h) in headers.iter().enumerate() {
            let h_norm = h.trim().to_lowercase();
            if h_norm == *alias {
                return Some(i);
            }
        }
    }

    None
}

pub fn parse_date(input: &str) -> Result<NaiveDate, AppError> {
    let clean = input.trim();
    if let Ok(d) = NaiveDate::parse_from_str(clean, "%d/%m/%Y") {
        return Ok(d);
    }
    if let Ok(d) = NaiveDate::parse_from_str(clean, "%Y-%m-%d") {
        return Ok(d);
    }
    if let Ok(d) = NaiveDate::parse_from_str(clean, "%d-%m-%Y") {
        return Ok(d);
    }
    Err(AppError::Validation(format!(
        "Formato de data não reconhecido: '{input}'. Use DD/MM/AAAA ou AAAA-MM-DD."
    )))
}

pub fn parse_amount(input: &str) -> Result<(Decimal, bool), AppError> {
    let clean = input
        .trim()
        .trim_start_matches("R$")
        .trim_start_matches("r$")
        .trim();

    let is_negative = clean.starts_with('-') || clean.starts_with('(');
    let unsigned = clean
        .trim_start_matches('-')
        .trim_start_matches('+')
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();

    let normalized = if unsigned.contains('.') && unsigned.contains(',') {
        unsigned.replace('.', "").replace(',', ".")
    } else if unsigned.contains(',') {
        unsigned.replace(',', ".")
    } else {
        unsigned.to_string()
    };

    let decimal = Decimal::from_str(&normalized)
        .map_err(|e| AppError::Validation(format!("Valor numérico inválido '{input}': {e}")))?;

    let positive_dec = decimal.abs();
    Ok((positive_dec, is_negative))
}
