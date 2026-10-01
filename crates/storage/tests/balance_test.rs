use app::{
    AccountService, BalanceService, CategoryService, CreateTransactionInput, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_balance_calculation_and_at_date_filter() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let balance_service = BalanceService::new(pool);

    // 1. Create accounts
    acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1500.00)).unwrap(),
        )
        .await
        .unwrap();

    acc_service
        .create_account(
            user_id,
            "Carteira".to_string(),
            AccountKind::Wallet,
            Money::from_decimal_non_negative(dec!(200.00)).unwrap(),
        )
        .await
        .unwrap();

    // 2. Create categories
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
            "Mercado".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 3. Transactions on Nubank
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(3000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Salário".to_string(),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(800.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
            description: "Mercado".to_string(),
        })
        .await
        .unwrap();

    // 4. Overall balance (no date filter)
    let report_all = balance_service.get_balance(user_id, None).await.unwrap();
    assert_eq!(report_all.accounts.len(), 2);

    let nubank_bal = report_all
        .accounts
        .iter()
        .find(|a| a.account_name == "Nubank")
        .unwrap();
    assert_eq!(nubank_bal.initial_balance.as_decimal(), dec!(1500.00));
    assert_eq!(nubank_bal.total_income.as_decimal(), dec!(3000.00));
    assert_eq!(nubank_bal.total_expense.as_decimal(), dec!(800.00));
    assert_eq!(nubank_bal.current_balance, dec!(3700.00));

    let carteira_bal = report_all
        .accounts
        .iter()
        .find(|a| a.account_name == "Carteira")
        .unwrap();
    assert_eq!(carteira_bal.current_balance, dec!(200.00));

    assert_eq!(report_all.total_initial_balance, dec!(1700.00));
    assert_eq!(report_all.total_income, dec!(3000.00));
    assert_eq!(report_all.total_expense, dec!(800.00));
    assert_eq!(report_all.total_balance, dec!(3900.00));

    // 5. Balance at 2026-10-10 (before the expense on Oct 15)
    let at_oct_10 = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
    let report_oct_10 = balance_service
        .get_balance(user_id, Some(at_oct_10))
        .await
        .unwrap();

    let nubank_oct_10 = report_oct_10
        .accounts
        .iter()
        .find(|a| a.account_name == "Nubank")
        .unwrap();
    assert_eq!(nubank_oct_10.total_expense.as_decimal(), dec!(0.00));
    assert_eq!(nubank_oct_10.current_balance, dec!(4500.00));
    assert_eq!(report_oct_10.total_balance, dec!(4700.00));
}
