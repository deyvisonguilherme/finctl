use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tui::{
    install_panic_hook, restore_terminal, run_tui, update, view, Command, Message, Model, Tab,
};

#[test]
fn test_initial_state_rendering_with_test_backend() {
    let backend = TestBackend::new(100, 25);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal de teste");

    let model = Model::new();
    terminal
        .draw(|f| view(&model, f))
        .expect("Deve desenhar view no TestBackend");

    let buffer = terminal.backend().buffer();

    // 1. Validar que o cabeçalho contém "finctl" e o sistema
    let mut buffer_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            buffer_text.push_str(cell.symbol());
        }
        buffer_text.push('\n');
    }

    assert!(
        buffer_text.contains("finctl"),
        "A tela inicial deve conter o título 'finctl'"
    );
    assert!(
        buffer_text.contains("Sistema de Controle Financeiro"),
        "A tela inicial deve conter a descrição do sistema"
    );

    // 2. Validar barra de abas
    assert!(
        buffer_text.contains("1: Dashboard"),
        "A tela deve exibir a aba Dashboard"
    );
    assert!(
        buffer_text.contains("2: Lançamentos"),
        "A tela deve exibir a aba Lançamentos"
    );
    assert!(
        buffer_text.contains("3: Relatórios"),
        "A tela deve exibir a aba Relatórios"
    );

    // 3. Validar atalhos no rodapé
    assert!(
        buffer_text.contains("q/Ctrl+C") || buffer_text.contains("Sair"),
        "O rodapé deve conter instrução para sair com q/Ctrl+C"
    );
    assert!(
        buffer_text.contains("Tab/1-3") || buffer_text.contains("Mudar Aba"),
        "O rodapé deve conter instrução de navegação de abas"
    );
}

#[test]
fn test_update_lifecycle_and_tab_navigation() {
    let mut model = Model::new();
    assert_eq!(model.active_tab, Tab::Dashboard);
    assert!(model.running);

    // Navegação com Tab (Next)
    let cmd = update(&mut model, Message::NextTab);
    assert_eq!(cmd, None);
    assert_eq!(model.active_tab, Tab::Transactions);

    let _ = update(&mut model, Message::NextTab);
    assert_eq!(model.active_tab, Tab::Reports);

    let _ = update(&mut model, Message::NextTab);
    assert_eq!(model.active_tab, Tab::Dashboard);

    // Navegação com BackTab (Previous)
    let _ = update(&mut model, Message::PreviousTab);
    assert_eq!(model.active_tab, Tab::Reports);

    // Seleção direta de abas
    let _ = update(&mut model, Message::SelectTab(1));
    assert_eq!(model.active_tab, Tab::Transactions);

    let _ = update(&mut model, Message::SelectTab(0));
    assert_eq!(model.active_tab, Tab::Dashboard);
}

#[test]
fn test_update_keyboard_shortcuts() {
    let mut model = Model::new();

    // Tecla 'q' deve solicitar saída
    let q_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(q_event));
    assert!(!model.running, "Pressionar 'q' deve parar a aplicação");

    // Reset
    model.running = true;

    // Ctrl+C deve solicitar saída
    let ctrl_c_event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let _ = update(&mut model, Message::Key(ctrl_c_event));
    assert!(!model.running, "Pressionar Ctrl+C deve parar a aplicação");

    // Reset
    model.running = true;

    // Tecla 'r' deve disparar comando RefreshData
    let r_event = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(r_event));
    assert_eq!(cmd, Some(Command::RefreshData));
    assert!(model.is_loading);
}

#[test]
fn test_async_message_handling() {
    let mut model = Model::new();

    // DataLoaded
    let _ = update(
        &mut model,
        Message::DataLoaded("Saldo total: R$ 5000,00".to_string()),
    );
    assert_eq!(
        model.data_summary.as_deref(),
        Some("Saldo total: R$ 5000,00")
    );
    assert!(!model.is_loading);
    assert!(model.status_message.contains("sucesso"));

    // ErrorOccurred
    let _ = update(
        &mut model,
        Message::ErrorOccurred("Falha de conexão com banco".to_string()),
    );
    assert!(model.status_message.contains("Falha de conexão"));
    assert!(!model.is_loading);
}

#[tokio::test]
async fn test_run_tui_graceful_shutdown_with_async_channel() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<Command>(8);
    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel::<Message>(8);

    let model = Model::new();

    // Em background: escuta o primeiro comando e envia Quit para encerrar o loop da TUI
    tokio::spawn(async move {
        // Recebe o FetchInitialData inicial
        if let Some(cmd) = cmd_rx.recv().await {
            assert_eq!(cmd, Command::FetchInitialData);
        }
        // Envia mensagem de encerramento
        let _ = msg_tx.send(Message::Quit).await;
    });

    let finished_model = run_tui(&mut terminal, model, Some(cmd_tx), Some(msg_rx))
        .await
        .expect("run_tui deve encerrar sem erro");

    assert!(
        !finished_model.running,
        "Aplicação deve ter finalizado após receber Quit"
    );
}

#[test]
fn test_panic_hook_and_restore_terminal_callable() {
    install_panic_hook();
    // restore_terminal pode ser chamado sem pânico mesmo sem modo raw ativo
    let res = restore_terminal();
    assert!(res.is_ok());
}
