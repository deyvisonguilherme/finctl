use app::{
    AccountService, CategoryService, CreateInstallmentsInput, DeleteInstallmentGroupSummary,
    EditInstallmentGroupInput, ListTransactionsInput, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_installments_workflow_split_dates_and_group_actions() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Setup account and categories
    let account = acc_service
        .create_account(
            user_id,
            "Cartão Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    let cat_eletronics = cat_service
        .create_category(
            user_id,
            "Eletrônicos".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_home = cat_service
        .create_category(user_id, "Casa".to_string(), TransactionKind::Expense, None)
        .await
        .unwrap();

    // 2. Create installment expense: R$ 100,00 in 3 installments starting on 2026-01-31 (D-05 test)
    let summary = tx_service
        .create_installments(CreateInstallmentsInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_eletronics.name.clone(),
            kind: TransactionKind::Expense,
            total_amount: Some(Money::new(dec!(100.00)).unwrap()),
            installment_amount: None,
            installments_count: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
            description: "Smartphone".to_string(),
        })
        .await
        .unwrap();

    let group_id = summary.installment_group_id;
    assert_eq!(summary.count, 3);
    assert_eq!(summary.total_amount, Money::new(dec!(100.00)).unwrap());
    assert_eq!(summary.transactions.len(), 3);

    // Verify rounding remainder went to first installment (33.34, 33.33, 33.33)
    assert_eq!(
        summary.transactions[0].amount,
        Money::new(dec!(33.34)).unwrap()
    );
    assert_eq!(
        summary.transactions[1].amount,
        Money::new(dec!(33.33)).unwrap()
    );
    assert_eq!(
        summary.transactions[2].amount,
        Money::new(dec!(33.33)).unwrap()
    );

    // Verify sum of installments is exactly equal to total amount
    let total_sum: rust_decimal::Decimal = summary
        .transactions
        .iter()
        .map(|t| t.amount.as_decimal())
        .sum();
    assert_eq!(total_sum, dec!(100.00));

    // Verify D-05 date handling (Jan 31, Feb 28, Mar 31)
    assert_eq!(
        summary.transactions[0].date,
        NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
    );
    assert_eq!(
        summary.transactions[1].date,
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
    assert_eq!(
        summary.transactions[2].date,
        NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
    );

    // Verify descriptions and installment fields
    assert_eq!(summary.transactions[0].description, "Smartphone (1/3)");
    assert_eq!(summary.transactions[0].installment_number, Some(1));
    assert_eq!(summary.transactions[0].installment_total, Some(3));
    assert_eq!(summary.transactions[0].status, TransactionStatus::Pending);

    // 3. Test list_transactions with --group
    let group_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            installment_group_id: Some(group_id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(group_txs.len(), 3);

    // 4. Mark first installment (1/3) as PAID
    let first_id = summary.transactions[0].id;
    let paid_tx = tx_service
        .pay_transaction(user_id, first_id, None)
        .await
        .unwrap();
    assert_eq!(paid_tx.status, TransactionStatus::Paid);

    // 5. Test Group Edit: edit category to 'Casa' and description to 'Celular Novo'
    let edit_summary = tx_service
        .edit_installment_group(EditInstallmentGroupInput {
            user_id,
            group_id,
            account_query: None,
            category_query: Some(cat_home.name.clone()),
            description: Some("Celular Novo".to_string()),
        })
        .await
        .unwrap();

    // 2 pending installments updated, 1 paid installment skipped
    assert_eq!(edit_summary.updated_count, 2);
    assert_eq!(edit_summary.skipped_paid_count, 1);

    // Verify paid installment (1/3) was NOT modified
    let tx1 = storage::TransactionRepository::find_by_id(pool, user_id, first_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tx1.description, "Smartphone (1/3)");
    assert_eq!(tx1.category_id, cat_eletronics.id);
    assert_eq!(tx1.status, TransactionStatus::Paid);

    // Verify pending installments (2/3 and 3/3) WERE modified
    let group_txs_after_edit = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            installment_group_id: Some(group_id),
            ..Default::default()
        })
        .await
        .unwrap();

    let tx2 = group_txs_after_edit
        .iter()
        .find(|t| t.installment_number == Some(2))
        .unwrap();
    assert_eq!(tx2.description, "Celular Novo (2/3)");
    assert_eq!(tx2.category_id, cat_home.id);
    assert_eq!(tx2.status, TransactionStatus::Pending);

    let tx3 = group_txs_after_edit
        .iter()
        .find(|t| t.installment_number == Some(3))
        .unwrap();
    assert_eq!(tx3.description, "Celular Novo (3/3)");
    assert_eq!(tx3.category_id, cat_home.id);
    assert_eq!(tx3.status, TransactionStatus::Pending);

    // 6. Test Group Delete: remove group
    let delete_summary = tx_service
        .delete_installment_group(user_id, group_id)
        .await
        .unwrap();

    assert_eq!(
        delete_summary,
        DeleteInstallmentGroupSummary {
            group_id,
            deleted_count: 2,
            skipped_paid_count: 1,
        }
    );

    // Verify paid installment is still intact
    let remaining_group_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            installment_group_id: Some(group_id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(remaining_group_txs.len(), 1);
    assert_eq!(remaining_group_txs[0].id, first_id);
    assert_eq!(remaining_group_txs[0].status, TransactionStatus::Paid);
}

#[tokio::test]
async fn test_installments_with_installment_amount() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    let account = acc_service
        .create_account(
            user_id,
            "Inter".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(10000.00)).unwrap(),
        )
        .await
        .unwrap();

    let cat = cat_service
        .create_category(
            user_id,
            "Cursos".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 10 installments of R$ 150,00 each
    let summary = tx_service
        .create_installments(CreateInstallmentsInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat.name.clone(),
            kind: TransactionKind::Expense,
            total_amount: None,
            installment_amount: Some(Money::new(dec!(150.00)).unwrap()),
            installments_count: 10,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
            description: "Curso Rust".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(summary.count, 10);
    assert_eq!(summary.total_amount, Money::new(dec!(1500.00)).unwrap());
    assert_eq!(summary.transactions.len(), 10);
    for tx in &summary.transactions {
        assert_eq!(tx.amount, Money::new(dec!(150.00)).unwrap());
        assert_eq!(tx.status, TransactionStatus::Pending);
    }
}
