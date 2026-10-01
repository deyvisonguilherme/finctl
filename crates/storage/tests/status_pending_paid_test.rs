use app::{
    AccountService, BalanceService, CategoryService, CreateTransactionInput, ListTransactionsInput,
    TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_status_pending_paid_workflow_and_balances() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let bal_service = BalanceService::new(pool);

    // 1. Setup account and categories
    let account = acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    let cat_salario = cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    let cat_aluguel = cat_service
        .create_category(
            user_id,
            "Aluguel".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_freela = cat_service
        .create_category(user_id, "Freela".to_string(), TransactionKind::Income, None)
        .await
        .unwrap();

    // 2. Create transactions:
    // Realized income: R$ 5.000,00
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_salario.name.clone(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(5000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Salário CLT".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Pending expense: R$ 1.200,00
    let pending_expense = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_aluguel.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
            description: "Aluguel Outubro".to_string(),
            status: Some(TransactionStatus::Pending),
        })
        .await
        .unwrap();

    // Pending income: R$ 800,00
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_freela.name.clone(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(800.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
            description: "Freela Design".to_string(),
            status: Some(TransactionStatus::Pending),
        })
        .await
        .unwrap();

    // 3. Verify Realized balance (should be 1000 + 5000 = 6000)
    let realized_bal = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(realized_bal.total_income, dec!(5000.00));
    assert_eq!(realized_bal.total_expense, dec!(0.00));
    assert_eq!(realized_bal.total_balance, dec!(6000.00));

    // 4. Verify Projected balance (should be 1000 + (5000 + 800) - 1200 = 5600)
    let projected_bal = bal_service.get_balance(user_id, None, true).await.unwrap();
    assert_eq!(projected_bal.total_income, dec!(5800.00));
    assert_eq!(projected_bal.total_expense, dec!(1200.00));
    assert_eq!(projected_bal.total_balance, dec!(5600.00));

    // 5. Test list filter by status
    let pending_list = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            status: Some(TransactionStatus::Pending),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(pending_list.len(), 2);

    let paid_list = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            status: Some(TransactionStatus::Paid),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(paid_list.len(), 1);

    // 6. Test pay_transaction on pending expense
    let new_pay_date = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let paid_expense = tx_service
        .pay_transaction(user_id, pending_expense.id, Some(new_pay_date))
        .await
        .unwrap();

    assert_eq!(paid_expense.status, TransactionStatus::Paid);
    assert_eq!(paid_expense.date, new_pay_date);

    // After paying expense, realized balance should be 1000 + 5000 - 1200 = 4800
    let realized_bal_2 = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(realized_bal_2.total_income, dec!(5000.00));
    assert_eq!(realized_bal_2.total_expense, dec!(1200.00));
    assert_eq!(realized_bal_2.total_balance, dec!(4800.00));

    // 7. Error handling: calling pay_transaction on already paid transaction must fail
    let err = tx_service
        .pay_transaction(user_id, pending_expense.id, None)
        .await
        .unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("já está marcado como realizado"));
}
