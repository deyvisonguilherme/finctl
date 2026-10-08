use app::{BackupInput, BackupService};
use clap::Args;
use sqlx::PgPool;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct BackupArgs {
    /// Diretório onde o arquivo de backup (.dump) será salvo
    #[arg(short, long)]
    pub output: PathBuf,

    /// Quantidade de backups mais recentes a manter (remove os excedentes mais antigos)
    #[arg(short, long)]
    pub keep: Option<usize>,
}

pub async fn handle_backup_command(
    args: BackupArgs,
    pool: &PgPool,
    database_url: &str,
) -> Result<(), (String, u8)> {
    let service = BackupService::new(pool);

    let summary = service
        .backup(BackupInput {
            database_url: database_url.to_string(),
            output_dir: args.output,
            keep: args.keep,
        })
        .await
        .map_err(|e| (e.to_string(), 2))?;

    println!("✓ Backup concluído com sucesso!");
    println!("  Arquivo: {}", summary.file_path.display());
    println!(
        "  Tamanho: {:.2} KB ({} bytes)",
        summary.file_size_bytes as f64 / 1024.0,
        summary.file_size_bytes
    );
    println!("  Duração: {:.2?}", summary.duration);

    if !summary.rotated_files.is_empty() {
        println!(
            "  Backups antigos removidos (rotação --keep {}):",
            args.keep.unwrap_or_default()
        );
        for rotated in summary.rotated_files {
            println!("    - {}", rotated.display());
        }
    }

    Ok(())
}
