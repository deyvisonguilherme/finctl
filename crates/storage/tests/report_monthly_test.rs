use app::{
    AccountService, CategoryService, CreateTransactionInput, MonthlyReportInput, ReportService,
    TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_report_monthly_scenarios() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let report_service = ReportService::new(pool);

    // 1. Setup accounts & categories
    acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(0.00)).unwrap(),
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
            "Aluguel".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 2. Transactions across months (2026-08, 2026-09, 2026-10)
    // 2026-08: Income 4000, Expense 1500 -> Net 2500, Savings 62.5%
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(4000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 8, 5).unwrap(),
            description: "Salário Ago".to_string(),
            status: None,
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Aluguel".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
            description: "Aluguel Ago".to_string(),
            status: None,
        })
        .await
        .unwrap();

    // 2026-09: Income 4500, Expense 2000 -> Net 2500, Savings 55.56%
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(4500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 5).unwrap(),
            description: "Salário Set".to_string(),
            status: None,
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Aluguel".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(2000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            description: "Aluguel Set".to_string(),
            status: None,
        })
        .await
        .unwrap();

    // Test specific month (2026-08)
    let rep_aug = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: Some("2026-08".to_string()),
            year: None,
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();

    assert_eq!(rep_aug.len(), 1);
    assert_eq!(rep_aug[0].month, "2026-08");
    assert_eq!(rep_aug[0].total_income.as_decimal(), dec!(4000.00));
    assert_eq!(rep_aug[0].total_expense.as_decimal(), dec!(1500.00));
    assert_eq!(rep_aug[0].net_balance, dec!(2500.00));
    assert_eq!(rep_aug[0].savings_rate, dec!(62.50));

    // Test full year (2026)
    let rep_year = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: None,
            year: Some(2026),
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();

    assert_eq!(rep_year.len(), 2);
    assert_eq!(rep_year[0].month, "2026-08");
    assert_eq!(rep_year[1].month, "2026-09");
}
