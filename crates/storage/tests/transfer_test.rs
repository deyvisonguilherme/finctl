use app::{
    AccountService, BalanceService, CategoryReportInput, CreateTransferInput, EditTransactionInput,
    MonthlyReportInput, ReportService, TransactionService, TransferService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_transfer_workflow() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let balance_service = BalanceService::new(pool);
    let report_service = ReportService::new(pool);
    let transfer_service = TransferService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Create two accounts: Checking (R$ 1000) and Savings (R$ 500)
    let acc_checking = acc_service
        .create_account(
            user_id,
            "Conta Corrente".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    let acc_savings = acc_service
        .create_account(
            user_id,
            "Poupança".to_string(),
            AccountKind::Savings,
            Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
        )
        .await
        .unwrap();

    // 2. Validate error when transfer from and to the same account
    let err_same_acc = transfer_service
        .create_transfer(CreateTransferInput {
            user_id,
            from_account_query: acc_checking.name.clone(),
            to_account_query: acc_checking.name.clone(),
            amount: Money::new(dec!(200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            description: None,
        })
        .await;
    assert!(err_same_acc.is_err());
    assert!(format!("{:?}", err_same_acc)
        .contains("As contas de origem e destino devem ser diferentes"));

    // 3. Perform transfer: R$ 300,00 from Checking to Savings
    let summary = transfer_service
        .create_transfer(CreateTransferInput {
            user_id,
            from_account_query: acc_checking.name.clone(),
            to_account_query: acc_savings.name.clone(),
            amount: Money::new(dec!(300.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            description: Some("Guardando na poupança".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(summary.from_account_name, "Conta Corrente");
    assert_eq!(summary.to_account_name, "Poupança");
    assert_eq!(summary.amount, Money::new(dec!(300.00)).unwrap());
    assert_eq!(summary.from_transaction.account_id, acc_checking.id);
    assert_eq!(summary.from_transaction.kind, TransactionKind::Expense);
    assert_eq!(summary.from_transaction.status, TransactionStatus::Paid);
    assert_eq!(
        summary.from_transaction.transfer_id,
        Some(summary.transfer_id)
    );
    assert_eq!(summary.to_transaction.account_id, acc_savings.id);
    assert_eq!(summary.to_transaction.kind, TransactionKind::Income);
    assert_eq!(summary.to_transaction.status, TransactionStatus::Paid);
    assert_eq!(
        summary.to_transaction.transfer_id,
        Some(summary.transfer_id)
    );

    // 4. Balances: Checking: 1000 - 300 = 700; Savings: 500 + 300 = 800; Total = 1500
    let balance_report = balance_service
        .get_balance(user_id, None, false)
        .await
        .unwrap();
    let checking_bal = balance_report
        .accounts
        .iter()
        .find(|a| a.account_id == acc_checking.id)
        .unwrap();
    let savings_bal = balance_report
        .accounts
        .iter()
        .find(|a| a.account_id == acc_savings.id)
        .unwrap();

    assert_eq!(checking_bal.current_balance, dec!(700.00));
    assert_eq!(savings_bal.current_balance, dec!(800.00));
    assert_eq!(balance_report.total_balance, dec!(1500.00));

    // 5. Reports: Monthly and Category reports must NOT include the transfer
    let monthly_report = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: Some("2026-04".to_string()),
            year: None,
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();
    assert!(monthly_report.is_empty());

    let cat_report = report_service
        .category_report(CategoryReportInput {
            user_id,
            month: Some("2026-04".to_string()),
            from_date: None,
            to_date: None,
            account_query: None,
            kind: None,
            depth: 1,
            include_pending: false,
        })
        .await
        .unwrap();
    assert!(cat_report.items.is_empty());
    assert_eq!(cat_report.total_amount.as_decimal(), dec!(0.00));

    // 6. tx edit should be blocked on transfer transactions
    let edit_res = tx_service
        .edit_transaction(EditTransactionInput {
            user_id,
            id: summary.from_transaction.id,
            amount: Some(Money::new(dec!(350.00)).unwrap()),
            date: None,
            category_query: None,
            account_query: None,
            description: None,
        })
        .await;
    assert!(edit_res.is_err());
    assert!(format!("{:?}", edit_res)
        .contains("Lançamentos de transferência não podem ser editados individualmente"));

    // 7. tx rm on one leg removes both legs
    tx_service
        .delete_transaction(user_id, summary.from_transaction.id)
        .await
        .unwrap();

    // Verify both transactions are gone
    let tx1 =
        storage::TransactionRepository::find_by_id(pool, user_id, summary.from_transaction.id)
            .await
            .unwrap();
    assert!(tx1.is_none());

    let tx2 = storage::TransactionRepository::find_by_id(pool, user_id, summary.to_transaction.id)
        .await
        .unwrap();
    assert!(tx2.is_none());

    // Verify balances restored: Checking 1000, Savings 500
    let restored_report = balance_service
        .get_balance(user_id, None, false)
        .await
        .unwrap();
    let checking_bal_restored = restored_report
        .accounts
        .iter()
        .find(|a| a.account_id == acc_checking.id)
        .unwrap();
    let savings_bal_restored = restored_report
        .accounts
        .iter()
        .find(|a| a.account_id == acc_savings.id)
        .unwrap();
    assert_eq!(checking_bal_restored.current_balance, dec!(1000.00));
    assert_eq!(savings_bal_restored.current_balance, dec!(500.00));
}
