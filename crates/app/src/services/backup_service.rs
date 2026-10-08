use crate::errors::AppError;
use sqlx::PgPool;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use storage::{create_database_if_not_exists, DatabaseConnectionInfo};

pub struct BackupInput {
    pub database_url: String,
    pub output_dir: PathBuf,
    pub keep: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct BackupSummary {
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub duration: std::time::Duration,
    pub rotated_files: Vec<PathBuf>,
}

pub struct RestoreInput {
    pub database_url: String,
    pub file_path: PathBuf,
    pub into_database: Option<String>,
    pub yes: bool,
}

#[derive(Debug, Clone)]
pub struct RestoreSummary {
    pub target_database: String,
    pub is_new_database: bool,
    pub new_database_url: Option<String>,
    pub duration: std::time::Duration,
}

pub struct BackupService<'a> {
    pool: &'a PgPool,
}

impl<'a> BackupService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub fn find_pg_dump() -> Result<PathBuf, AppError> {
        find_binary("pg_dump", "FINCTL_PG_DUMP_PATH").ok_or_else(|| {
            AppError::Infrastructure(
                "O utilitário 'pg_dump' não foi encontrado no PATH do sistema. \
                 Instale o pacote 'postgresql-client' ou defina a variável FINCTL_PG_DUMP_PATH."
                    .to_string(),
            )
        })
    }

    pub fn find_pg_restore() -> Result<PathBuf, AppError> {
        find_binary("pg_restore", "FINCTL_PG_RESTORE_PATH").ok_or_else(|| {
            AppError::Infrastructure(
                "O utilitário 'pg_restore' não foi encontrado no PATH do sistema. \
                 Instale o pacote 'postgresql-client' ou defina a variável FINCTL_PG_RESTORE_PATH."
                    .to_string(),
            )
        })
    }

    pub async fn backup(&self, input: BackupInput) -> Result<BackupSummary, AppError> {
        let pg_dump_bin = Self::find_pg_dump()?;
        let conn_info = DatabaseConnectionInfo::parse(&input.database_url)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        // Garantir diretório de saída
        if !input.output_dir.exists() {
            fs::create_dir_all(&input.output_dir).map_err(|e| {
                AppError::Infrastructure(format!(
                    "Não foi possível criar o diretório de backup '{}': {e}",
                    input.output_dir.display()
                ))
            })?;
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let final_filename = format!("finctl_backup_{timestamp}.dump");
        let final_path = input.output_dir.join(&final_filename);
        let temp_path = input.output_dir.join(format!("{final_filename}.tmp"));

        let start_time = Instant::now();

        // Executar pg_dump gravando no arquivo .tmp
        let mut cmd = Command::new(&pg_dump_bin);
        cmd.env("PGHOST", &conn_info.host)
            .env("PGPORT", conn_info.port.to_string())
            .env("PGUSER", &conn_info.username);

        if let Some(pwd) = &conn_info.password {
            cmd.env("PGPASSWORD", pwd);
        }

        cmd.arg("-Fc")
            .arg("-d")
            .arg(&conn_info.database)
            .arg("-f")
            .arg(&temp_path);

        let output = cmd.output().map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            AppError::Infrastructure(format!("Falha ao invocar pg_dump: {e}"))
        })?;

        if !output.status.success() {
            let _ = fs::remove_file(&temp_path);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Infrastructure(format!(
                "pg_dump falhou com código {:?}: {}",
                output.status.code(),
                stderr.trim()
            )));
        }

        // Renomear arquivo temporário para o destino final (atômico)
        fs::rename(&temp_path, &final_path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            AppError::Infrastructure(format!("Falha ao finalizar arquivo de backup: {e}"))
        })?;

        let file_size_bytes = fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);

        let duration = start_time.elapsed();

        // Rotação de backups mais antigos (--keep N)
        let mut rotated_files = Vec::new();
        if let Some(keep) = input.keep {
            if keep > 0 {
                rotated_files = rotate_backups(&input.output_dir, keep)?;
            }
        }

        Ok(BackupSummary {
            file_path: final_path,
            file_size_bytes,
            duration,
            rotated_files,
        })
    }

    pub async fn restore(&self, input: RestoreInput) -> Result<RestoreSummary, AppError> {
        let pg_restore_bin = Self::find_pg_restore()?;

        if !input.file_path.exists() || !input.file_path.is_file() {
            return Err(AppError::Validation(format!(
                "Arquivo de backup '{}' não encontrado ou não é um arquivo válido.",
                input.file_path.display()
            )));
        }

        let conn_info = DatabaseConnectionInfo::parse(&input.database_url)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let start_time = Instant::now();

        let (target_database, is_new_database, new_database_url) = match input.into_database {
            Some(target) => {
                let is_current = target == conn_info.database;
                if is_current && !input.yes {
                    return Err(AppError::Validation(format!(
                        "Restauração sobre o banco atual ('{target}') requer confirmação. Utilize a flag --yes."
                    )));
                }

                if !is_current {
                    create_database_if_not_exists(self.pool, &target).await?;
                }

                let new_url = if is_current {
                    None
                } else {
                    Some(conn_info.with_database(&target))
                };

                (target, !is_current, new_url)
            }
            None => {
                // Padrão: restaura em um banco novo com timestamp
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let new_db_name = format!("{}_restore_{timestamp}", conn_info.database);
                create_database_if_not_exists(self.pool, &new_db_name).await?;
                let new_url = conn_info.with_database(&new_db_name);
                (new_db_name, true, Some(new_url))
            }
        };

        let mut cmd = Command::new(&pg_restore_bin);
        cmd.env("PGHOST", &conn_info.host)
            .env("PGPORT", conn_info.port.to_string())
            .env("PGUSER", &conn_info.username);

        if let Some(pwd) = &conn_info.password {
            cmd.env("PGPASSWORD", pwd);
        }

        // Flags para restauração limpa e sem conflitos de roles
        cmd.arg("--clean")
            .arg("--if-exists")
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("-d")
            .arg(&target_database)
            .arg(&input.file_path);

        let output = cmd
            .output()
            .map_err(|e| AppError::Infrastructure(format!("Falha ao invocar pg_restore: {e}")))?;

        // pg_restore pode retornar aviso não-fatal com código 0 ou 1 quando objetos são dropados
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignorar erros secundários de DROP IF EXISTS se objetos não existiam
            let has_fatal = output.status.code().unwrap_or(1) > 1
                || (stderr.contains("FATAL:") || stderr.contains("PANIC:"));
            if has_fatal {
                return Err(AppError::Infrastructure(format!(
                    "pg_restore falhou com código {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                )));
            }
        }

        let duration = start_time.elapsed();

        Ok(RestoreSummary {
            target_database,
            is_new_database,
            new_database_url,
            duration,
        })
    }
}

fn rotate_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| {
        AppError::Infrastructure(format!(
            "Falha ao ler diretório '{}' para rotação: {e}",
            dir.display()
        ))
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if file_name.starts_with("finctl_backup_") && file_name.ends_with(".dump") {
                    files.push(path);
                }
            }
        }
    }

    // Ordenar de forma decrescente pelo nome (timestamp YYYYMMDD_HHMMSS é ordenável)
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    let mut removed = Vec::new();
    if files.len() > keep {
        for old_file in &files[keep..] {
            if fs::remove_file(old_file).is_ok() {
                removed.push(old_file.clone());
            }
        }
    }

    Ok(removed)
}

fn find_binary(name: &str, env_var: &str) -> Option<PathBuf> {
    if let Ok(path_str) = std::env::var(env_var) {
        let p = PathBuf::from(path_str);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = candidate.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Some(candidate);
                        }
                    }
                }
                #[cfg(not(unix))]
                return Some(candidate);
            }
        }
    }

    None
}
