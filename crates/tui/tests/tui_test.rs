use app::{
    AccountBalance, BalanceReport, CategoryBudgetStatus, DashboardData, MonthlySummary,
    UpcomingDueItem, UpcomingKind,
};
use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use domain::{AccountId, AccountKind, BudgetIndicator, CategoryId, Money};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rust_decimal_macros::dec;
use tui::{
    install_panic_hook, restore_terminal, run_tui, update, view, Command, Message, Model, Tab,
};

fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn test_initial_state_rendering_with_test_backend() {
    let backend = TestBackend::new(100, 25);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal de teste");

    let model = Model::new();
    terminal
        .draw(|f| view(&model, f))
        .expect("Deve desenhar view no TestBackend");

    let buffer_text = buffer_to_string(&terminal);

    // 1. Validar que o cabeçalho contém "finctl" e o sistema
    assert!(
        buffer_text.contains("finctl"),
        "A tela inicial deve conter o título 'finctl'"
    );
    assert!(
        buffer_text.contains("Sistema de Controle Financeiro"),
        "A tela inicial deve conter a descrição do sistema"
    );

    // 2. Validar barra de 5 abas
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
    assert!(
        buffer_text.contains("4: Orçamentos"),
        "A tela deve exibir a aba Orçamentos"
    );
    assert!(
        buffer_text.contains("5: Metas"),
        "A tela deve exibir a aba Metas"
    );

    // 3. Validar atalhos no rodapé
    assert!(
        buffer_text.contains("q/Ctrl+C") || buffer_text.contains("Sair"),
        "O rodapé deve conter instrução para sair com q/Ctrl+C"
    );
    assert!(
        buffer_text.contains("Tab/1-5") || buffer_text.contains("Mudar Aba"),
        "O rodapé deve conter instrução de navegação de abas [Tab/1-5]"
    );
}

#[test]
fn test_dashboard_rendering_in_80x24_with_mock_data() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal de teste");

    let mut model = Model::new();

    let data = DashboardData {
        balance_report: BalanceReport {
            as_of_date: None,
            projected: false,
            accounts: vec![
                AccountBalance {
                    account_id: AccountId::generate(),
                    account_name: "Nubank".to_string(),
                    account_kind: AccountKind::Checking,
                    initial_balance: Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
                    total_income: Money::from_decimal_non_negative(dec!(2000.00)).unwrap(),
                    total_expense: Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
                    current_balance: dec!(2500.00),
                },
                AccountBalance {
                    account_id: AccountId::generate(),
                    account_name: "Carteira".to_string(),
                    account_kind: AccountKind::Wallet,
                    initial_balance: Money::from_decimal_non_negative(dec!(50.00)).unwrap(),
                    total_income: Money::from_decimal_non_negative(dec!(100.00)).unwrap(),
                    total_expense: Money::from_decimal_non_negative(dec!(0.00)).unwrap(),
                    current_balance: dec!(150.00),
                },
            ],
            total_initial_balance: dec!(1050.00),
            total_income: dec!(2100.00),
            total_expense: dec!(500.00),
            total_balance: dec!(2650.00),
        },
        monthly_summary: MonthlySummary {
            month: "2026-10".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(4000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(1800.00)).unwrap(),
            net_balance: dec!(2200.00),
            savings_rate: dec!(55.0),
        },
        budget_statuses: vec![CategoryBudgetStatus {
            category_id: CategoryId::generate(),
            category_name: "Alimentação".to_string(),
            budget_amount: Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
            consumed_amount: Money::from_decimal_non_negative(dec!(750.00)).unwrap(),
            remaining_amount: dec!(250.00),
            percentage: dec!(75.0),
            indicator: BudgetIndicator::Ok,
            is_monthly_exception: false,
        }],
        upcoming_items: vec![
            UpcomingDueItem {
                due_date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
                description: "Internet Fibra".to_string(),
                amount: Money::from_decimal_non_negative(dec!(120.00)).unwrap(),
                kind: UpcomingKind::Expense,
                is_overdue: false,
                account_name: Some("Nubank".to_string()),
            },
            UpcomingDueItem {
                due_date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
                description: "Energia Elétrica".to_string(),
                amount: Money::from_decimal_non_negative(dec!(230.00)).unwrap(),
                kind: UpcomingKind::Expense,
                is_overdue: true,
                account_name: Some("Nubank".to_string()),
            },
        ],
    };

    update(&mut model, Message::DashboardLoaded(Box::new(data)));

    terminal
        .draw(|f| view(&model, f))
        .expect("Deve desenhar dashboard populado em 80x24");

    let text = buffer_to_string(&terminal);

    // 1. Quadrante Saldos por Conta
    assert!(text.contains("Saldos por Conta"));
    assert!(text.contains("Nubank"));
    assert!(text.contains("Total Geral:"));
    assert!(text.contains("2.650,00"));

    // 2. Quadrante Resumo do Mês
    assert!(text.contains("Resumo do Mês (2026-10)"));
    assert!(text.contains("Receitas:"));
    assert!(text.contains("Despesas:"));
    assert!(text.contains("Saldo Líquido:"));
    assert!(text.contains("2.200,00"));
    assert!(text.contains("55.0%"));

    // 3. Quadrante Status dos Orçamentos
    assert!(text.contains("Status dos Orçamentos"));
    assert!(text.contains("Alimentação"));
    assert!(text.contains("75%"));

    // 4. Quadrante Próximos Vencimentos
    assert!(text.contains("Próximos Vencimentos"));
    assert!(text.contains("Internet"));
    assert!(text.contains("ATRASADO"));
}

#[test]
fn test_dashboard_loading_and_error_states() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();

    // 1. Estado de Carregamento
    update(&mut model, Message::SetLoading(true));
    terminal.draw(|f| view(&model, f)).unwrap();
    let loading_text = buffer_to_string(&terminal);
    assert!(loading_text.contains("Carregando dados financeiros..."));
    assert!(loading_text.contains("Sincronizando..."));

    // 2. Estado de Erro sem fechar a TUI
    update(
        &mut model,
        Message::ErrorOccurred("Falha ao conectar no host postgres:5432".to_string()),
    );
    assert!(
        model.running,
        "A TUI deve continuar em execução mesmo com erro"
    );
    terminal.draw(|f| view(&model, f)).unwrap();
    let error_text = buffer_to_string(&terminal);
    assert!(error_text.contains("Falha na Sincronização"));
    assert!(error_text.contains("Falha ao conectar no host postgres:5432"));
    assert!(error_text.contains("ERRO:"));

    // 3. Recuperação com tecla 'r'
    let r_event = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(r_event));
    assert_eq!(cmd, Some(Command::RefreshData));
    assert!(model.is_loading);
    assert_eq!(model.error_message, None);
}

#[test]
fn test_update_lifecycle_and_5_tab_navigation() {
    let mut model = Model::new();
    assert_eq!(model.active_tab, Tab::Dashboard);
    assert!(model.running);

    // Navegação com Tab (Next) por todas as 5 abas
    let cmd = update(&mut model, Message::NextTab);
    assert_eq!(cmd, None);
    assert_eq!(model.active_tab, Tab::Transactions);

    let _ = update(&mut model, Message::NextTab);
    assert_eq!(model.active_tab, Tab::Reports);

    let _ = update(&mut model, Message::NextTab);
    assert_eq!(model.active_tab, Tab::Budgets);

    let _ = update(&mut model, Message::NextTab);
    assert_eq!(model.active_tab, Tab::Goals);

    let _ = update(&mut model, Message::NextTab);
    assert_eq!(model.active_tab, Tab::Dashboard);

    // Navegação com BackTab (Previous)
    let _ = update(&mut model, Message::PreviousTab);
    assert_eq!(model.active_tab, Tab::Goals);

    let _ = update(&mut model, Message::PreviousTab);
    assert_eq!(model.active_tab, Tab::Budgets);

    // Seleção direta de teclas 1 a 5
    let event_1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(event_1));
    assert_eq!(model.active_tab, Tab::Dashboard);

    let event_4 = KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(event_4));
    assert_eq!(model.active_tab, Tab::Budgets);

    let event_5 = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(event_5));
    assert_eq!(model.active_tab, Tab::Goals);
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

#[tokio::test]
async fn test_run_tui_graceful_shutdown_with_async_channel() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<Command>(8);
    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel::<Message>(8);

    let model = Model::new();

    tokio::spawn(async move {
        if let Some(cmd) = cmd_rx.recv().await {
            assert_eq!(cmd, Command::FetchInitialData);
        }
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
    let res = restore_terminal();
    assert!(res.is_ok());
}
