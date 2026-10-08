use crate::cli::TuiArgs;
use domain::UserId;
use sqlx::PgPool;

pub async fn handle_tui_command(
    _args: TuiArgs,
    pool: &PgPool,
    user_id: UserId,
) -> Result<(), (String, u8)> {
    // 1. Instalar hook de pânico para restaurar terminal em caso de crash
    tui::install_panic_hook();

    // 2. Inicializar o terminal no modo raw com tela alternada
    let mut terminal = tui::init_terminal()
        .map_err(|e| (format!("Falha ao inicializar terminal da TUI: {e}"), 2))?;

    // 3. Configurar canais de comunicação bidirecionais
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<tui::Command>(32);
    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel::<tui::Message>(32);

    // 4. Executar tarefas assíncronas de banco em background sem travar a interface
    let pool_clone = pool.clone();
    let bg_task = tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                tui::Command::FetchInitialData | tui::Command::RefreshData => {
                    let _ = msg_tx.send(tui::Message::SetLoading(true)).await;
                    let balance_service = app::BalanceService::new(&pool_clone);
                    match balance_service.get_balance(user_id, None, false).await {
                        Ok(report) => {
                            let summary = format!(
                                "Contas: {} | Saldo Total: R$ {:.2}",
                                report.accounts.len(),
                                report.total_balance
                            );
                            let _ = msg_tx.send(tui::Message::DataLoaded(summary)).await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .send(tui::Message::ErrorOccurred(err.to_string()))
                                .await;
                        }
                    }
                }
                tui::Command::Custom(s) => {
                    let _ = msg_tx.send(tui::Message::StatusMessage(s)).await;
                }
            }
        }
    });

    // 5. Executar o ciclo de eventos da TUI
    let model = tui::Model::new();
    let run_res = tui::run_tui(&mut terminal, model, Some(cmd_tx), Some(msg_rx)).await;

    // 6. Restaurar o terminal original
    let _ = tui::restore_terminal();
    bg_task.abort();

    run_res.map_err(|e| (format!("Erro na execução da TUI: {e}"), 2))?;
    Ok(())
}
