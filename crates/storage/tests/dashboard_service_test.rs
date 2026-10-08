use app::{
    AccountService, BalanceService, BudgetService, CategoryService, CreateTransactionInput,
    DashboardService, MonthlyReportInput, ReportService, TransactionService, UpcomingKind,
};
use chrono::NaiveDate;
use domain::{AccountKind, BudgetIndicator, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_dashboard_service_data_consistency() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let balance_service = BalanceService::new(pool);
    let report_service = ReportService::new(pool);
    let budget_service = BudgetService::new(pool);
    let dashboard_service = DashboardService::new(pool);

    // 1. Criar contas (Corrente e Cartão de Crédito)
    let _checking = acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    let _card = acc_service
        .create_account_with_input(app::CreateAccountInput {
            user_id,
            name: "Cartão Nubank".to_string(),
            kind: AccountKind::CreditCard,
            initial_balance: Money::from_decimal_non_negative(dec!(0.00)).unwrap(),
            closing_day: Some(20),
            due_day: Some(27),
            credit_limit: Some(Money::from_decimal_non_negative(dec!(5000.00)).unwrap()),
        })
        .await
        .unwrap();

    // 2. Criar categorias
    let _cat_salario = cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    let _cat_alimentacao = cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let _cat_contas = cat_service
        .create_category(
            user_id,
            "Contas".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 3. Criar transações pagas
    // Receita: 3000.00
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::from_decimal_non_negative(dec!(3000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Salário Empresa".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Despesa em conta corrente: 400.00
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(400.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
            description: "Supermercado".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Despesa no cartão de crédito: 350.00 (mês 2026-10)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Cartão Nubank".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(350.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            description: "Restaurante".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // 4. Criar transações pendentes para o bloco de Próximos Vencimentos
    // Pendente futura
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Contas".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(150.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
            description: "Internet Fibra".to_string(),
            status: Some(TransactionStatus::Pending),
        })
        .await
        .unwrap();

    // Pendente atrasada (no passado)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Contas".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(80.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            description: "Água Antiga".to_string(),
            status: Some(TransactionStatus::Pending),
        })
        .await
        .unwrap();

    // 5. Configurar orçamento
    budget_service
        .set_budget(
            user_id,
            "Alimentação".to_string(),
            Money::from_decimal_non_negative(dec!(800.00)).unwrap(),
            Some("2026-10".to_string()),
        )
        .await
        .unwrap();

    // 6. Consultar dados do DashboardService
    let dashboard = dashboard_service
        .get_dashboard_data(user_id, Some("2026-10"))
        .await
        .unwrap();

    // 7. Validar equivalência estrita com BalanceService
    let direct_balance = balance_service
        .get_balance(user_id, None, false)
        .await
        .unwrap();
    assert_eq!(
        dashboard.balance_report.total_balance, direct_balance.total_balance,
        "Total balance do dashboard deve bater com BalanceService"
    );
    assert_eq!(
        dashboard.balance_report.accounts.len(),
        direct_balance.accounts.len(),
        "Quantidade de contas deve bater com BalanceService"
    );

    // 8. Validar equivalência estrita com ReportService (monthly_report)
    let direct_monthly = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            include_pending: false,
            ..Default::default()
        })
        .await
        .unwrap();
    let monthly_item = direct_monthly
        .iter()
        .find(|i| i.month == "2026-10")
        .expect("Deve haver relatório para 2026-10");

    assert_eq!(
        dashboard.monthly_summary.total_income, monthly_item.total_income,
        "Receita mensal deve bater com report monthly"
    );
    assert_eq!(
        dashboard.monthly_summary.total_expense, monthly_item.total_expense,
        "Despesa mensal deve bater com report monthly"
    );
    assert_eq!(
        dashboard.monthly_summary.net_balance, monthly_item.net_balance,
        "Saldo líquido mensal deve bater com report monthly"
    );

    // 9. Validar equivalência com BudgetService (get_budget_status)
    let direct_budgets = budget_service
        .get_budget_status(user_id, "2026-10".to_string())
        .await
        .unwrap();
    assert_eq!(
        dashboard.budget_statuses.len(),
        direct_budgets.len(),
        "Quantidade de orçamentos deve bater com BudgetService"
    );
    let alimentacao_status = dashboard
        .budget_statuses
        .iter()
        .find(|b| b.category_name == "Alimentação")
        .expect("Orçamento de Alimentação deve estar presente");
    assert_eq!(
        alimentacao_status.budget_amount,
        Money::from_decimal_non_negative(dec!(800.00)).unwrap()
    );
    assert_eq!(
        alimentacao_status.consumed_amount,
        Money::from_decimal_non_negative(dec!(750.00)).unwrap()
    );
    assert_eq!(alimentacao_status.indicator, BudgetIndicator::Warning);

    // 10. Validar lista de Próximos Vencimentos
    assert!(
        !dashboard.upcoming_items.is_empty(),
        "Deve conter itens pendentes ou faturas de cartão"
    );

    // A despesa atrasada de setembro deve estar marcada como is_overdue = true
    let atrasada = dashboard
        .upcoming_items
        .iter()
        .find(|i| i.description == "Água Antiga")
        .expect("Deve conter o lançamento atrasado");
    assert!(
        atrasada.is_overdue,
        "Item do passado deve ser marcado como atrasado"
    );
    assert_eq!(atrasada.kind, UpcomingKind::Expense);

    // A despesa de outubro
    let futura = dashboard
        .upcoming_items
        .iter()
        .find(|i| i.description == "Internet Fibra")
        .expect("Deve conter a despesa futura");
    assert_eq!(
        futura.amount,
        Money::from_decimal_non_negative(dec!(150.00)).unwrap()
    );

    // Fatura do cartão de crédito
    let fatura = dashboard
        .upcoming_items
        .iter()
        .find(|i| i.kind == UpcomingKind::CardInvoice)
        .expect("Deve conter a fatura do cartão");
    assert_eq!(
        fatura.amount,
        Money::from_decimal_non_negative(dec!(350.00)).unwrap()
    );
}
