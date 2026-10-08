use app::{
    AccountBalance, BalanceReport, CategoryBudgetStatus, DashboardData, MonthlySummary,
    PaginatedTransactions, ReportsScreenData, TransactionDetails, UpcomingDueItem, UpcomingKind,
};
use chrono::{NaiveDate, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use domain::{
    AccountId, AccountKind, BudgetIndicator, CategoryComparisonReport, CategoryComparisonRow,
    CategoryId, CategoryReportItem, CategoryReportSummary, Money, MonthlyReportItem, TransactionId,
    TransactionKind, TransactionStatus, UserId,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rust_decimal_macros::dec;
use tui::{
    generate_help_text, install_panic_hook, restore_terminal, run_tui, update, view, Command,
    Message, Model, ReportSubView, ShortcutRegistry, Tab, Theme, ThemeMode,
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
    assert!(matches!(cmd, Some(Command::FetchTransactions(_))));
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

fn sample_transaction(
    desc: &str,
    amount: domain::Money,
    kind: TransactionKind,
    status: TransactionStatus,
) -> TransactionDetails {
    TransactionDetails {
        id: TransactionId::generate(),
        user_id: UserId::generate(),
        account_id: AccountId::generate(),
        account_name: "Nubank".to_string(),
        category_id: CategoryId::generate(),
        category_name: "Alimentação".to_string(),
        kind,
        amount,
        date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
        description: desc.to_string(),
        status,
        transfer_id: None,
        installment_group_id: None,
        installment_number: None,
        installment_total: None,
        recurring_rule_id: None,
        import_hash: None,
        reconciled_at: None,
        tags: vec!["teste".to_string()],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        deleted_at: None,
    }
}

#[test]
fn test_transactions_table_rendering_and_pagination() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();
    model.active_tab = Tab::Transactions;

    let tx1 = sample_transaction(
        "Supermercado",
        Money::from_decimal_non_negative(dec!(150.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Paid,
    );
    let tx2 = sample_transaction(
        "Salário Mensal",
        Money::from_decimal_non_negative(dec!(3500.00)).unwrap(),
        TransactionKind::Income,
        TransactionStatus::Paid,
    );
    let tx3 = sample_transaction(
        "Conta de Luz",
        Money::from_decimal_non_negative(dec!(120.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Pending,
    );

    let paginated = PaginatedTransactions {
        items: vec![tx1, tx2, tx3],
        total_count: 45,
        page: 1,
        page_size: 15,
        total_pages: 3,
    };

    update(&mut model, Message::TransactionsLoaded(paginated));

    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);

    // 1. Validar colunas e dados renderizados
    assert!(text.contains("Lançamentos"));
    assert!(text.contains("Supermercado"));
    assert!(text.contains("Salário Mensal"));
    assert!(text.contains("Conta de Luz"));
    assert!(text.contains("150,00"));
    assert!(text.contains("3.500,00"));
    assert!(text.contains("[Pago]"));
    assert!(text.contains("[Pend]"));

    // 2. Validar indicador de paginação
    assert!(text.contains("Página 1 de 3 (Total: 45 lançamentos)"));

    // 3. Testar navegação para próxima página com tecla ']'
    let next_page_event = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(next_page_event));
    assert!(matches!(cmd, Some(Command::FetchTransactions(_))));
    assert_eq!(model.transactions_state.page, 2);

    // 4. Testar navegação para página anterior com tecla '['
    let prev_page_event = KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(prev_page_event));
    assert!(matches!(cmd, Some(Command::FetchTransactions(_))));
    assert_eq!(model.transactions_state.page, 1);
}

#[test]
fn test_transactions_multi_selection_and_pay_flow() {
    let mut model = Model::new();
    model.active_tab = Tab::Transactions;

    let tx1 = sample_transaction(
        "Internet",
        Money::from_decimal_non_negative(dec!(100.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Pending,
    );
    let tx2 = sample_transaction(
        "Celular",
        Money::from_decimal_non_negative(dec!(50.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Pending,
    );

    let id1 = tx1.id;
    let id2 = tx2.id;

    let paginated = PaginatedTransactions {
        items: vec![tx1, tx2],
        total_count: 2,
        page: 1,
        page_size: 15,
        total_pages: 1,
    };
    update(&mut model, Message::TransactionsLoaded(paginated));

    // Selecionar primeiro item com Space
    let space_event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(space_event));
    assert!(model.transactions_state.selected_ids.contains(&id1));

    // Descer cursor e selecionar segundo item
    let down_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(down_event));
    assert_eq!(model.transactions_state.cursor_index, 1);

    let space_event_2 = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(space_event_2));
    assert!(model.transactions_state.selected_ids.contains(&id2));
    assert_eq!(model.transactions_state.selected_ids.len(), 2);

    // Pressionar tecla 'p' para pagar os itens selecionados
    let pay_event = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(pay_event));

    match cmd {
        Some(Command::PayTransactions(ids, date)) => {
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&id1));
            assert!(ids.contains(&id2));
            assert_eq!(date, None);
        }
        other => panic!("Esperado Command::PayTransactions, obteve: {:?}", other),
    }
}

#[test]
fn test_transactions_form_modal_add_and_validation() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();
    model.active_tab = Tab::Transactions;

    // 1. Pressionar 'a' para abrir formulário de criação
    let a_event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(a_event));
    assert!(model.transactions_state.form_modal.is_some());

    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Adicionar Lançamento"));

    // 2. Tentar salvar vazio (Enter) -> deve dar erro de validação (Conta vazia)
    let enter_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(enter_event));
    assert_eq!(cmd, None);
    assert!(model
        .transactions_state
        .form_modal
        .as_ref()
        .unwrap()
        .validation_error
        .is_some());

    // 3. Preencher dados válidos
    if let Some(form) = model.transactions_state.form_modal.as_mut() {
        form.account_input = "Nubank".to_string();
        form.category_input = "Mercado".to_string();
        form.amount_input = "150.00".to_string();
        form.date_input = "2026-10-15".to_string();
        form.description_input = "Compras Semanais".to_string();
    }

    // 4. Salvar com Enter -> deve gerar Command::CreateTransaction
    let cmd_save = update(&mut model, Message::Key(enter_event));
    match cmd_save {
        Some(Command::CreateTransaction(input)) => {
            assert_eq!(input.account_query, "Nubank");
            assert_eq!(input.category_query, "Mercado");
            assert_eq!(input.description, "Compras Semanais");
            assert_eq!(
                input.amount,
                Money::from_decimal_non_negative(dec!(150.00)).unwrap()
            );
        }
        other => panic!("Esperado Command::CreateTransaction, obteve: {:?}", other),
    }

    // 5. Esc cancela e fecha o modal
    let _ = update(&mut model, Message::Key(a_event));
    assert!(model.transactions_state.form_modal.is_some());
    let esc_event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(esc_event));
    assert!(model.transactions_state.form_modal.is_none());
}

#[test]
fn test_transactions_search_and_filters_flow() {
    let mut model = Model::new();
    model.active_tab = Tab::Transactions;

    // 1. Pressionar '/' para ativar busca
    let slash_event = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(slash_event));
    assert!(model.transactions_state.is_searching);

    // 2. Digitar termo "aluguel"
    for ch in "aluguel".chars() {
        let char_event = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
        let _ = update(&mut model, Message::Key(char_event));
    }
    assert_eq!(model.transactions_state.search_query, "aluguel");

    // 3. Enter confirma a busca e dispara FetchTransactions
    let enter_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(enter_event));
    match cmd {
        Some(Command::FetchTransactions(input)) => {
            assert_eq!(input.search_description.as_deref(), Some("aluguel"));
        }
        other => panic!("Esperado FetchTransactions com busca, obteve: {:?}", other),
    }
    assert!(!model.transactions_state.is_searching);

    // 4. Pressionar 'f' para abrir modal de filtros
    let f_event = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(f_event));
    assert!(model.transactions_state.filter_modal.is_some());

    // Preencher filtro de mês
    if let Some(modal) = model.transactions_state.filter_modal.as_mut() {
        modal.month_input = "2026-10".to_string();
    }
    let cmd_filter = update(&mut model, Message::Key(enter_event));
    match cmd_filter {
        Some(Command::FetchTransactions(input)) => {
            assert_eq!(input.month.as_deref(), Some("2026-10"));
        }
        other => panic!(
            "Esperado FetchTransactions com filtro de mês, obteve: {:?}",
            other
        ),
    }
    assert!(model.transactions_state.filter_modal.is_none());
}

#[test]
fn test_transactions_delete_confirmation_modal() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();
    model.active_tab = Tab::Transactions;

    let tx = sample_transaction(
        "Gasto Cancelado",
        Money::from_decimal_non_negative(dec!(99.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Paid,
    );
    let id = tx.id;
    let paginated = PaginatedTransactions {
        items: vec![tx],
        total_count: 1,
        page: 1,
        page_size: 15,
        total_pages: 1,
    };
    update(&mut model, Message::TransactionsLoaded(paginated));

    // 1. Pressionar 'd' abre modal de confirmação
    let d_event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(d_event));
    assert!(model.transactions_state.delete_confirm.is_some());

    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Confirmar Exclusão"));
    assert!(text.contains("Gasto Cancelado"));

    // 2. Pressionar 's' confirma a exclusão
    let s_event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(s_event));
    match cmd {
        Some(Command::DeleteTransactions(ids)) => {
            assert_eq!(ids, vec![id]);
        }
        other => panic!("Esperado Command::DeleteTransactions, obteve: {:?}", other),
    }
    assert!(model.transactions_state.delete_confirm.is_none());
}

fn sample_reports_data() -> ReportsScreenData {
    let cat_item1 = CategoryReportItem {
        category_id: CategoryId::generate(),
        category_name: "Alimentação e Supermercados Longo".to_string(),
        parent_id: None,
        parent_name: None,
        kind: TransactionKind::Expense,
        total_amount: Money::from_decimal_non_negative(dec!(1500.00)).unwrap(),
        transaction_count: 10,
        percentage: dec!(60.0),
    };
    let cat_item2 = CategoryReportItem {
        category_id: CategoryId::generate(),
        category_name: "Transporte".to_string(),
        parent_id: None,
        parent_name: None,
        kind: TransactionKind::Expense,
        total_amount: Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        transaction_count: 5,
        percentage: dec!(40.0),
    };

    let category_report = CategoryReportSummary {
        kind: Some(TransactionKind::Expense),
        total_amount: Money::from_decimal_non_negative(dec!(2500.00)).unwrap(),
        items: vec![cat_item1, cat_item2],
    };

    let monthly_history = vec![
        MonthlyReportItem {
            month: "2026-05".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(3000.00)).unwrap(),
            net_balance: dec!(2000.00),
            savings_rate: dec!(40.0),
        },
        MonthlyReportItem {
            month: "2026-06".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(5200.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(3100.00)).unwrap(),
            net_balance: dec!(2100.00),
            savings_rate: dec!(40.4),
        },
        MonthlyReportItem {
            month: "2026-07".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(3500.00)).unwrap(),
            net_balance: dec!(1500.00),
            savings_rate: dec!(30.0),
        },
        MonthlyReportItem {
            month: "2026-08".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(5500.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(3200.00)).unwrap(),
            net_balance: dec!(2300.00),
            savings_rate: dec!(41.8),
        },
        MonthlyReportItem {
            month: "2026-09".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(2800.00)).unwrap(),
            net_balance: dec!(2200.00),
            savings_rate: dec!(44.0),
        },
        MonthlyReportItem {
            month: "2026-10".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(6000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(2500.00)).unwrap(),
            net_balance: dec!(3500.00),
            savings_rate: dec!(58.3),
        },
    ];

    let comp_row1 = CategoryComparisonRow {
        category_name: "Alimentação".to_string(),
        kind: TransactionKind::Expense,
        monthly_amounts: vec![
            (
                "2026-09".to_string(),
                Money::from_decimal_non_negative(dec!(1200.00)).unwrap(),
            ),
            (
                "2026-10".to_string(),
                Money::from_decimal_non_negative(dec!(1500.00)).unwrap(),
            ),
        ],
        absolute_diff: dec!(300.00),
        percent_diff: Some(dec!(25.0)),
    };
    let comp_row2 = CategoryComparisonRow {
        category_name: "Transporte".to_string(),
        kind: TransactionKind::Expense,
        monthly_amounts: vec![
            (
                "2026-09".to_string(),
                Money::from_decimal_non_negative(dec!(1200.00)).unwrap(),
            ),
            (
                "2026-10".to_string(),
                Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
            ),
        ],
        absolute_diff: dec!(-200.00),
        percent_diff: Some(dec!(-16.7)),
    };

    let comparison_categories = CategoryComparisonReport {
        months: vec!["2026-09".to_string(), "2026-10".to_string()],
        rows: vec![comp_row1, comp_row2],
    };

    let comparison_totals = vec![
        MonthlyReportItem {
            month: "2026-09".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(2800.00)).unwrap(),
            net_balance: dec!(2200.00),
            savings_rate: dec!(44.0),
        },
        MonthlyReportItem {
            month: "2026-10".to_string(),
            total_income: Money::from_decimal_non_negative(dec!(6000.00)).unwrap(),
            total_expense: Money::from_decimal_non_negative(dec!(2500.00)).unwrap(),
            net_balance: dec!(3500.00),
            savings_rate: dec!(58.3),
        },
    ];

    ReportsScreenData {
        reference_month: "2026-10".to_string(),
        include_pending: false,
        category_report,
        monthly_history,
        comparison_categories,
        comparison_totals,
    }
}

#[test]
fn test_reports_categories_view_rendering_and_truncation() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();
    model.active_tab = Tab::Reports;
    model.reports_state.active_subview = ReportSubView::Categories;
    model.reports_state.reference_month = "2026-10".to_string();

    let data = sample_reports_data();
    update(&mut model, Message::ReportDataLoaded(Box::new(data)));

    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);

    // 1. Validar barra de controle e título
    assert!(text.contains("Relatórios Financeiros"));
    assert!(text.contains("[1] Categorias"));
    assert!(text.contains("◄ 2026-10 ►"));

    // 2. Validar cabeçalho e dados de categorias
    assert!(text.contains("Gastos por Categoria — 2026-10"));
    assert!(text.contains("Total: R$ 2.500,00"));

    // 3. Validar truncamento da categoria longa com '…'
    assert!(text.contains("Alimentação e …"));
    assert!(text.contains("Transporte"));

    // 4. Validar percentual, valores em pt-BR e barra de distribuição
    assert!(text.contains("60,0%"));
    assert!(text.contains("1.500,00"));
    assert!(text.contains("40,0%"));
    assert!(text.contains("1.000,00"));
    assert!(text.contains("█"));
    assert!(text.contains("░"));
}

#[test]
fn test_reports_evolution_view_sparkline_and_table() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();
    model.active_tab = Tab::Reports;
    let data = sample_reports_data();
    update(&mut model, Message::ReportDataLoaded(Box::new(data)));

    // Alternar para sub-visão 2: Evolução Mensal via tecla '2'
    let event_2 = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(event_2));
    assert_eq!(cmd, None);
    assert_eq!(
        model.reports_state.active_subview,
        ReportSubView::MonthlyEvolution
    );

    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);

    // Validar painéis de Sparklines
    assert!(text.contains("Evolução Mensal"));
    assert!(text.contains("Receitas (Últimos 6 Meses)"));
    assert!(text.contains("Despesas (Últimos 6 Meses)"));

    // Validar tabela resumo detalhada mês a mês
    assert!(text.contains("Histórico Mês a Mês"));
    assert!(text.contains("2026-05"));
    assert!(text.contains("2026-10"));
    assert!(text.contains("6.000,00"));
    assert!(text.contains("2.500,00"));
    assert!(text.contains("+R$ 3.500,00"));
    assert!(text.contains("58,3%"));
}

#[test]
fn test_reports_comparison_view_and_deltas() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut model = Model::new();
    model.active_tab = Tab::Reports;
    let data = sample_reports_data();
    update(&mut model, Message::ReportDataLoaded(Box::new(data)));

    // Alternar para sub-visão 3: Comparativo via tecla '3'
    let event_3 = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
    let cmd = update(&mut model, Message::Key(event_3));
    assert_eq!(cmd, None);
    assert_eq!(
        model.reports_state.active_subview,
        ReportSubView::Comparison
    );

    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);

    // Validar resumo dos totais gerais
    assert!(text.contains("Comparativo"));
    assert!(text.contains("Comparativo dos Totais Gerais"));
    assert!(text.contains("Receitas:"));
    assert!(text.contains("Despesas:"));
    assert!(text.contains("Saldo:"));

    // Validar tabela de categorias com deltas nominais e percentuais
    assert!(text.contains("Variação de Despesas por Categoria"));
    assert!(text.contains("Alimentação"));
    assert!(text.contains("+R$ 300,00"));
    assert!(text.contains("+25,0%"));
    assert!(text.contains("Transporte"));
    assert!(text.contains("-R$ 200,00"));
    assert!(text.contains("-16,7%"));
}

#[test]
fn test_reports_navigation_month_and_pending_toggle_and_period_modal() {
    let mut model = Model::new();
    model.active_tab = Tab::Reports;
    model.reports_state.reference_month = "2026-10".to_string();
    model.reports_state.include_pending = false;

    // 1. Tecla '[' retrocede o mês para 2026-09
    let prev_month_event = KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE);
    let cmd_prev = update(&mut model, Message::Key(prev_month_event));
    match cmd_prev {
        Some(Command::FetchReportData {
            month,
            include_pending,
        }) => {
            assert_eq!(month, "2026-09");
            assert!(!include_pending);
        }
        other => panic!("Esperado FetchReportData, obteve: {:?}", other),
    }
    assert_eq!(model.reports_state.reference_month, "2026-09");

    // 2. Tecla ']' avança o mês de volta para 2026-10
    let next_month_event = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
    let cmd_next = update(&mut model, Message::Key(next_month_event));
    match cmd_next {
        Some(Command::FetchReportData {
            month,
            include_pending,
        }) => {
            assert_eq!(month, "2026-10");
            assert!(!include_pending);
        }
        other => panic!("Esperado FetchReportData, obteve: {:?}", other),
    }
    assert_eq!(model.reports_state.reference_month, "2026-10");

    // 3. Tecla 'i' alterna a inclusão de previstos (toggle ON/OFF)
    let toggle_pending_event = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    let cmd_toggle = update(&mut model, Message::Key(toggle_pending_event));
    match cmd_toggle {
        Some(Command::FetchReportData {
            month,
            include_pending,
        }) => {
            assert_eq!(month, "2026-10");
            assert!(include_pending);
        }
        other => panic!(
            "Esperado FetchReportData com pending=true, obteve: {:?}",
            other
        ),
    }
    assert!(model.reports_state.include_pending);

    // 4. Tecla 'p' abre o modal de período
    let p_event = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
    let _ = update(&mut model, Message::Key(p_event));
    assert!(model.reports_state.period_modal.is_some());

    // Digitar entrada inválida e tentar submeter com Enter
    if let Some(modal) = model.reports_state.period_modal.as_mut() {
        modal.input_month = "invalido".to_string();
    }
    let enter_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let cmd_invalid = update(&mut model, Message::Key(enter_event));
    assert_eq!(cmd_invalid, None);
    assert!(model
        .reports_state
        .period_modal
        .as_ref()
        .unwrap()
        .validation_error
        .is_some());

    // Digitar mês válido "2026-03" e confirmar
    if let Some(modal) = model.reports_state.period_modal.as_mut() {
        modal.input_month = "2026-03".to_string();
    }
    let cmd_valid = update(&mut model, Message::Key(enter_event));
    match cmd_valid {
        Some(Command::FetchReportData {
            month,
            include_pending,
        }) => {
            assert_eq!(month, "2026-03");
            assert!(include_pending);
        }
        other => panic!("Esperado FetchReportData 2026-03, obteve: {:?}", other),
    }
    assert!(model.reports_state.period_modal.is_none());
    assert_eq!(model.reports_state.reference_month, "2026-03");
}

fn set_deterministic_clock(model: &mut Model) {
    let naive = NaiveDate::from_ymd_opt(2026, 10, 8)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    model.last_tick =
        chrono::DateTime::from_naive_utc_and_offset(naive, *chrono::Local::now().offset());
}

fn create_mock_dashboard_model() -> Model {
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
    model.dashboard_data = Some(data);
    model
}

fn create_mock_transactions_model() -> Model {
    let mut model = Model::new();
    model.active_tab = Tab::Transactions;
    let tx1 = sample_transaction(
        "Supermercado",
        Money::from_decimal_non_negative(dec!(150.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Paid,
    );
    let tx2 = sample_transaction(
        "Salário Mensal",
        Money::from_decimal_non_negative(dec!(3500.00)).unwrap(),
        TransactionKind::Income,
        TransactionStatus::Paid,
    );
    let tx3 = sample_transaction(
        "Conta de Luz",
        Money::from_decimal_non_negative(dec!(120.00)).unwrap(),
        TransactionKind::Expense,
        TransactionStatus::Pending,
    );
    let paginated = PaginatedTransactions {
        items: vec![tx1, tx2, tx3],
        total_count: 3,
        page: 1,
        page_size: 15,
        total_pages: 1,
    };
    model.transactions_state.items = paginated.items;
    model.transactions_state.total_count = paginated.total_count;
    model.transactions_state.page = paginated.page;
    model.transactions_state.page_size = paginated.page_size;
    model.transactions_state.total_pages = paginated.total_pages;
    model
}

fn create_mock_reports_model(subview: ReportSubView) -> Model {
    let mut model = Model::new();
    model.active_tab = Tab::Reports;
    model.reports_state.active_subview = subview;
    model.reports_state.reference_month = "2026-10".to_string();
    model.reports_state.data = Some(sample_reports_data());
    model
}

#[test]
fn test_all_shortcuts_appear_in_help_panel() {
    let mut model = Model::new();
    model.is_help_open = true;

    // 1. Validar que cada atalho registrado aparece no texto gerado para a tela de ajuda
    let all_shortcuts = ShortcutRegistry::all();
    let help_text = generate_help_text(&model);

    for sc in &all_shortcuts {
        assert!(
            help_text.contains(sc.key),
            "A tecla '{}' deve estar presente no texto do painel de ajuda",
            sc.key
        );
        assert!(
            help_text.contains(sc.description),
            "A descrição '{}' deve estar presente no texto do painel de ajuda",
            sc.description
        );
    }

    // 2. Renderizar no TestBackend com altura suficiente (100x60) para garantir desenho visual no buffer
    let backend = TestBackend::new(100, 60);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal de teste");

    terminal
        .draw(|f| view(&model, f))
        .expect("Deve renderizar view");
    let buffer_text = buffer_to_string(&terminal);

    for sc in &all_shortcuts {
        assert!(
            buffer_text.contains(sc.key),
            "Buffer do terminal deve conter a tecla '{}'",
            sc.key
        );
    }
}

#[test]
fn test_help_modal_lifecycle_navigation_and_closing() {
    let mut model = Model::new();
    assert!(!model.is_help_open);

    // 1. Abrir com tecla '?'
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
    );
    assert!(model.is_help_open);
    assert_eq!(model.help_scroll, 0);

    // 2. Rolar para baixo com 'j'
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
    );
    assert_eq!(model.help_scroll, 1);

    // 3. Rolar para cima com 'k'
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)),
    );
    assert_eq!(model.help_scroll, 0);

    // 4. Fechar com 'Esc'
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(!model.is_help_open);

    // 5. Reabrir com ToggleHelp e fechar com '?'
    let _ = update(&mut model, Message::ToggleHelp);
    assert!(model.is_help_open);
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
    );
    assert!(!model.is_help_open);
}

#[test]
fn test_theme_toggle_and_shortcuts() {
    let mut model = Model::new();
    assert_eq!(model.theme.mode, ThemeMode::Dark);

    // 1. Alternar tema com tecla 't'
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)),
    );
    assert_eq!(model.theme.mode, ThemeMode::Light);
    assert!(model.status_message.contains("Claro"));

    // 2. Alternar de volta com mensagem ToggleTheme
    let _ = update(&mut model, Message::ToggleTheme);
    assert_eq!(model.theme.mode, ThemeMode::Dark);
    assert!(model.status_message.contains("Escuro"));

    // 3. Definir tema explicitamente
    let _ = update(&mut model, Message::SetTheme(ThemeMode::Light));
    assert_eq!(model.theme.mode, ThemeMode::Light);
}

#[test]
fn test_contextual_footer_tips_per_tab_and_modal() {
    let mut model = Model::new();
    let backend = TestBackend::new(100, 25);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal de teste");

    // Dashboard
    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Tab/1-5") || text.contains("Mudar Aba"));
    assert!(text.contains("Atualizar"));
    assert!(text.contains("Ajuda"));

    // Lançamentos
    model.active_tab = Tab::Transactions;
    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Novo"));
    assert!(text.contains("Edit"));
    assert!(text.contains("Excl"));
    assert!(text.contains("Pagar"));

    // Lançamentos com busca ativa
    model.transactions_state.is_searching = true;
    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Busca") || text.contains("Digitar"));

    // Relatórios
    model.transactions_state.is_searching = false;
    model.active_tab = Tab::Reports;
    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Visão") || text.contains("Mês"));

    // Ajuda aberta
    model.is_help_open = true;
    terminal.draw(|f| view(&model, f)).unwrap();
    let text = buffer_to_string(&terminal);
    assert!(text.contains("Rolar"));
    assert!(text.contains("Fechar Ajuda"));
}

#[test]
fn test_snapshot_dashboard() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_dashboard_model();
    set_deterministic_clock(&mut model);
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("dashboard_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_transactions_table() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_transactions_model();
    set_deterministic_clock(&mut model);
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("transactions_table_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_transactions_add_modal() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_transactions_model();
    set_deterministic_clock(&mut model);
    // Abrir modal de criação
    let _ = update(
        &mut model,
        Message::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
    );
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("transactions_modal_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_reports_categories() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_reports_model(ReportSubView::Categories);
    set_deterministic_clock(&mut model);
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("reports_categories_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_reports_evolution() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_reports_model(ReportSubView::MonthlyEvolution);
    set_deterministic_clock(&mut model);
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("reports_evolution_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_reports_comparison() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_reports_model(ReportSubView::Comparison);
    set_deterministic_clock(&mut model);
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("reports_comparison_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_help_modal() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_dashboard_model();
    set_deterministic_clock(&mut model);
    model.is_help_open = true;
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("help_modal_80x24", buffer_to_string(&terminal));
}

#[test]
fn test_snapshot_light_theme() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Deve criar terminal");
    let mut model = create_mock_dashboard_model();
    model.theme = Theme::light();
    set_deterministic_clock(&mut model);
    terminal.draw(|f| view(&model, f)).unwrap();
    insta::assert_snapshot!("light_theme_80x24", buffer_to_string(&terminal));
}
