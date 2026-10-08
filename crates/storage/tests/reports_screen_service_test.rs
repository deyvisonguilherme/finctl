use app::{
    AccountService, CategoryService, CreateTransactionInput, ReportService, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_reports_screen_data_integration() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let report_service = ReportService::new(pool);

    // 1. Criar conta e categorias
    acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    cat_service
        .create_category(
            user_id,
            "Transporte".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 2. Criar lançamentos em 2026-09
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(5000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 5).unwrap(),
            description: "Salário Setembro".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            description: "Mercado Setembro".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // 3. Criar lançamentos em 2026-10 (Realizados e Pendentes)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(6000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Salário Outubro".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            description: "Mercado Outubro".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Transporte".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            description: "Combustível Pendente".to_string(),
            status: Some(TransactionStatus::Pending),
        })
        .await
        .unwrap();

    // 4. Testar get_reports_screen_data sem previstos (include_pending: false)
    let reports_paid = report_service
        .get_reports_screen_data(user_id, "2026-10", false)
        .await
        .unwrap();

    assert_eq!(reports_paid.reference_month, "2026-10");
    assert!(!reports_paid.include_pending);

    // Gastos por categoria em 2026-10 apenas pagos: apenas Alimentação (R$ 1.500,00)
    assert_eq!(
        reports_paid.category_report.total_amount,
        Money::new(dec!(1500.00)).unwrap()
    );
    assert_eq!(reports_paid.category_report.items.len(), 1);
    assert_eq!(
        reports_paid.category_report.items[0].category_name,
        "Alimentação"
    );
    assert_eq!(
        reports_paid.category_report.items[0].percentage,
        dec!(100.0)
    );

    // Histórico de 6 meses
    assert_eq!(reports_paid.monthly_history.len(), 6);
    assert_eq!(reports_paid.monthly_history[5].month, "2026-10");
    assert_eq!(
        reports_paid.monthly_history[5].total_income,
        Money::new(dec!(6000.00)).unwrap()
    );
    assert_eq!(
        reports_paid.monthly_history[5].total_expense,
        Money::new(dec!(1500.00)).unwrap()
    );
    assert_eq!(reports_paid.monthly_history[5].net_balance, dec!(4500.00));

    // Comparativo (2026-09 vs 2026-10)
    assert_eq!(reports_paid.comparison_totals.len(), 2);
    let sept_tot = &reports_paid.comparison_totals[0];
    let oct_tot = &reports_paid.comparison_totals[1];
    assert_eq!(sept_tot.total_expense, Money::new(dec!(1200.00)).unwrap());
    assert_eq!(oct_tot.total_expense, Money::new(dec!(1500.00)).unwrap());

    // 5. Testar get_reports_screen_data COM previstos (include_pending: true)
    let reports_all = report_service
        .get_reports_screen_data(user_id, "2026-10", true)
        .await
        .unwrap();

    assert!(reports_all.include_pending);
    // Gastos com previstos: Alimentação (1500) + Transporte (500) = 2000
    assert_eq!(
        reports_all.category_report.total_amount,
        Money::new(dec!(2000.00)).unwrap()
    );
    assert_eq!(reports_all.category_report.items.len(), 2);
}
