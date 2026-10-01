use app::CategoryService;
use clap::Args;
use domain::UserId;
use sqlx::PgPool;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Inicializa o banco sem criar as categorias padrão
    #[arg(long = "no-seed")]
    pub no_seed: bool,
}

pub async fn handle_init_command(
    args: InitArgs,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    // 1. Aplicar migrações
    storage::run_migrations(pool)
        .await
        .map_err(|e| (format!("Erro ao aplicar migrações: {e}"), 2))?;
    println!("Banco de dados inicializado e migrações aplicadas com sucesso!");

    // 2. Criar seed de categorias se não especificado --no-seed
    if !args.no_seed {
        let service = CategoryService::new(pool);
        let count = service
            .seed_default_categories(user_id)
            .await
            .map_err(|e| (format!("Erro ao semear categorias: {e}"), 2))?;

        if count > 0 {
            println!("{count} categorias e subcategorias padrão foram criadas com sucesso!");
        } else {
            println!("Categorias padrão já estavam configuradas.");
        }
    }

    println!("\nfinctl está pronto para uso! Experimente criar sua primeira conta com `finctl account add`.");
    Ok(())
}
