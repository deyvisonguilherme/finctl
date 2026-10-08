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
                    let dashboard_service = app::DashboardService::new(&pool_clone);
                    match dashboard_service.get_dashboard_data(user_id, None).await {
                        Ok(data) => {
                            let _ = msg_tx
                                .send(tui::Message::DashboardLoaded(Box::new(data)))
                                .await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .send(tui::Message::ErrorOccurred(err.to_string()))
                                .await;
                        }
                    }
                }
                tui::Command::FetchTransactions(mut input) => {
                    let _ = msg_tx.send(tui::Message::SetLoading(true)).await;
                    input.user_id = user_id;
                    let tx_service = app::TransactionService::new(&pool_clone);
                    match tx_service.list_transactions_paginated(input).await {
                        Ok(paginated) => {
                            let _ = msg_tx
                                .send(tui::Message::TransactionsLoaded(paginated))
                                .await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .send(tui::Message::ErrorOccurred(err.to_string()))
                                .await;
                        }
                    }
                }
                tui::Command::PayTransactions(ids, date) => {
                    let _ = msg_tx.send(tui::Message::SetLoading(true)).await;
                    let tx_service = app::TransactionService::new(&pool_clone);
                    match tx_service
                        .pay_multiple_transactions(user_id, &ids, date)
                        .await
                    {
                        Ok(count) => {
                            let _ = msg_tx
                                .send(tui::Message::TransactionActionSuccess(format!(
                                    "{} lançamento(s) marcado(s) como pago(s)!",
                                    count
                                )))
                                .await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .send(tui::Message::ErrorOccurred(err.to_string()))
                                .await;
                        }
                    }
                }
                tui::Command::DeleteTransactions(ids) => {
                    let _ = msg_tx.send(tui::Message::SetLoading(true)).await;
                    let tx_service = app::TransactionService::new(&pool_clone);
                    match tx_service.delete_multiple_transactions(user_id, &ids).await {
                        Ok(count) => {
                            let _ = msg_tx
                                .send(tui::Message::TransactionActionSuccess(format!(
                                    "{} lançamento(s) removido(s) com sucesso!",
                                    count
                                )))
                                .await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .send(tui::Message::ErrorOccurred(err.to_string()))
                                .await;
                        }
                    }
                }
                tui::Command::CreateTransaction(mut input) => {
                    let _ = msg_tx.send(tui::Message::SetLoading(true)).await;
                    input.user_id = user_id;
                    let tx_service = app::TransactionService::new(&pool_clone);
                    match tx_service.create_transaction(input).await {
                        Ok(tx) => {
                            let _ = msg_tx
                                .send(tui::Message::TransactionActionSuccess(format!(
                                    "Lançamento '{}' criado com sucesso!",
                                    tx.description
                                )))
                                .await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .send(tui::Message::ErrorOccurred(err.to_string()))
                                .await;
                        }
                    }
                }
                tui::Command::EditTransaction(mut input) => {
                    let _ = msg_tx.send(tui::Message::SetLoading(true)).await;
                    input.user_id = user_id;
                    let tx_service = app::TransactionService::new(&pool_clone);
                    match tx_service.edit_transaction(input).await {
                        Ok(tx) => {
                            let _ = msg_tx
                                .send(tui::Message::TransactionActionSuccess(format!(
                                    "Lançamento '{}' atualizado com sucesso!",
                                    tx.description
                                )))
                                .await;
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
