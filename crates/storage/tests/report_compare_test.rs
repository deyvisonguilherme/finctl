use app::{
    AccountService, CategoryService, CompareCategoriesInput, CreateTransactionInput, ReportService,
    TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_report_compare_scenarios() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let report_service = ReportService::new(pool);

    // 1. Create account & categories
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
            "Mercado".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    cat_service
        .create_category(user_id, "Lazer".to_string(), TransactionKind::Expense, None)
        .await
        .unwrap();

    // 2. Transactions:
    // 2026-08: Mercado R$ 500,00; Lazer R$ 0 (no tx)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
            description: "Mercado Ago".to_string(),
        })
        .await
        .unwrap();

    // 2026-09: Mercado R$ 600,00 (+20%); Lazer R$ 200,00 (base zero -> n/d)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(600.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            description: "Mercado Set".to_string(),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Lazer".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
            description: "Lazer Set".to_string(),
        })
        .await
        .unwrap();

    // Test comparison between 2026-08 and 2026-09
    let report = report_service
        .compare_categories(CompareCategoriesInput {
            user_id,
            months: Some(vec!["2026-08".to_string(), "2026-09".to_string()]),
            last_n: None,
            kind: Some(TransactionKind::Expense),
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();

    assert_eq!(report.months, vec!["2026-08", "2026-09"]);
    assert_eq!(report.rows.len(), 2);

    let row_lazer = report
        .rows
        .iter()
        .find(|r| r.category_name == "Lazer")
        .unwrap();
    assert_eq!(row_lazer.monthly_amounts[0].1.as_decimal(), dec!(0.00));
    assert_eq!(row_lazer.monthly_amounts[1].1.as_decimal(), dec!(200.00));
    assert_eq!(row_lazer.absolute_diff, dec!(200.00));
    assert_eq!(row_lazer.percent_diff, None); // Base zero: n/d

    let row_mercado = report
        .rows
        .iter()
        .find(|r| r.category_name == "Mercado")
        .unwrap();
    assert_eq!(row_mercado.monthly_amounts[0].1.as_decimal(), dec!(500.00));
    assert_eq!(row_mercado.monthly_amounts[1].1.as_decimal(), dec!(600.00));
    assert_eq!(row_mercado.absolute_diff, dec!(100.00));
    assert_eq!(row_mercado.percent_diff, Some(dec!(20.00))); // +20%
}
