use app::{
    AccountService, BalanceService, CategoryService, CreateInstallmentsInput,
    CreateTransactionInput, ListTransactionsInput, ReportService, TransactionService,
    TransferService,
};
use chrono::{Duration, NaiveDate, Utc};
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_transaction_soft_delete_and_restore() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let bal_service = BalanceService::new(pool);
    let rep_service = ReportService::new(pool);

    acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .expect("criar conta");

    cat_service
        .create_category(
            user_id,
            "Mercado".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .expect("criar categoria");

    let tx = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Compras".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .expect("criar transacao");

    // Check active balance: 1000 - 200 = 800
    let bal = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(bal.total_balance, dec!(800.00));

    // Check active report: 200 expense
    let rep = rep_service
        .monthly_report(app::MonthlyReportInput {
            user_id,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(rep.len(), 1);
    assert_eq!(rep[0].total_expense.as_decimal(), dec!(200.00));

    // Soft delete transaction
    tx_service
        .delete_transaction(user_id, tx.id)
        .await
        .expect("soft delete tx");

    // List normal transactions -> empty
    let active_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            deleted: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active_txs.len(), 0);

    // List deleted transactions -> 1 item
    let deleted_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            deleted: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted_txs.len(), 1);
    assert_eq!(deleted_txs[0].id, tx.id);

    // Check balance after delete -> 1000 (expense ignored)
    let bal_after = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(bal_after.total_balance, dec!(1000.00));

    // Check monthly report after delete -> 0 expense
    let rep_after = rep_service
        .monthly_report(app::MonthlyReportInput {
            user_id,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(rep_after.len(), 0);

    // Restore transaction
    let restored = tx_service
        .restore_transaction(user_id, tx.id, false)
        .await
        .expect("restaurar tx");
    assert_eq!(restored, 1);

    // Check balance after restore -> 800
    let bal_restored = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(bal_restored.total_balance, dec!(800.00));
}

#[tokio::test]
async fn test_transfer_soft_delete_and_restore() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let tx_service = TransactionService::new(pool);
    let transfer_service = TransferService::new(pool);

    acc_service
        .create_account(
            user_id,
            "Conta Corrente".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    acc_service
        .create_account(
            user_id,
            "Poupança".to_string(),
            AccountKind::Savings,
            Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
        )
        .await
        .unwrap();

    let transfer = transfer_service
        .create_transfer(app::CreateTransferInput {
            user_id,
            from_account_query: "Conta Corrente".to_string(),
            to_account_query: "Poupança".to_string(),
            amount: Money::new(dec!(300.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: Some("Transferencia teste".to_string()),
        })
        .await
        .expect("criar transferencia");

    // Delete one leg (source tx)
    tx_service
        .delete_transaction(user_id, transfer.from_transaction.id)
        .await
        .expect("delete transfer");

    // Verify both legs are soft deleted
    let active_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            deleted: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active_txs.len(), 0);

    let deleted_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            deleted: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted_txs.len(), 2);

    // Restore one leg (dest tx)
    let restored = tx_service
        .restore_transaction(user_id, transfer.to_transaction.id, false)
        .await
        .expect("restore transfer");
    assert_eq!(restored, 2);

    // Verify both legs are active again
    let active_after = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            deleted: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active_after.len(), 2);
}

#[tokio::test]
async fn test_installment_group_soft_delete_and_restore() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

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
            "Eletrônicos".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let inst_summary = tx_service
        .create_installments(CreateInstallmentsInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Eletrônicos".to_string(),
            kind: TransactionKind::Expense,
            total_amount: Some(Money::new(dec!(300.00)).unwrap()),
            installment_amount: None,
            installments_count: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Fone".to_string(),
        })
        .await
        .unwrap();

    let first_tx_id = inst_summary.transactions[0].id;
    let second_tx_id = inst_summary.transactions[1].id;

    // Pay the 1st installment
    let mut tx0 = inst_summary.transactions[0].clone();
    tx0.status = TransactionStatus::Paid;
    storage::TransactionRepository::update(pool, &tx0)
        .await
        .unwrap();

    // Delete group (should only delete pending: installments 2 and 3)
    let del_summary = tx_service
        .delete_installment_group(user_id, inst_summary.installment_group_id)
        .await
        .unwrap();
    assert_eq!(del_summary.deleted_count, 2);
    assert_eq!(del_summary.skipped_paid_count, 1);

    // Verify active txs has 1 (the paid one)
    let active_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            installment_group_id: Some(inst_summary.installment_group_id),
            deleted: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active_txs.len(), 1);
    assert_eq!(active_txs[0].id, first_tx_id);

    // Verify deleted txs has 2 (installments 2 and 3)
    let deleted_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            installment_group_id: Some(inst_summary.installment_group_id),
            deleted: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted_txs.len(), 2);

    // Restore group via second_tx_id with group=true
    let restored = tx_service
        .restore_transaction(user_id, second_tx_id, true)
        .await
        .unwrap();
    assert_eq!(restored, 2);

    // Verify all 3 are active now
    let active_restored = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            installment_group_id: Some(inst_summary.installment_group_id),
            deleted: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active_restored.len(), 3);
}

#[tokio::test]
async fn test_account_and_category_deletion_guards_and_soft_delete() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    acc_service
        .create_account(
            user_id,
            "Itaú".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
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

    let tx = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Itaú".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(50.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Lanche".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Try deleting account -> blocked
    let acc_err = acc_service.delete_account(user_id, "Itaú").await;
    assert!(acc_err.is_err());
    assert!(acc_err
        .unwrap_err()
        .to_string()
        .contains("existem transações ativas"));

    // Try deleting category -> blocked
    let cat_err = cat_service.delete_category(user_id, "Alimentação").await;
    assert!(cat_err.is_err());
    assert!(cat_err
        .unwrap_err()
        .to_string()
        .contains("existem transações ativas"));

    // Soft delete transaction
    tx_service.delete_transaction(user_id, tx.id).await.unwrap();

    // Now deleting account succeeds
    let acc_del = acc_service.delete_account(user_id, "Itaú").await;
    assert!(acc_del.is_ok());

    // Now deleting category succeeds
    let cat_del = cat_service.delete_category(user_id, "Alimentação").await;
    assert!(cat_del.is_ok());

    // Account list should be empty
    let accounts = acc_service.list_accounts(user_id).await.unwrap();
    assert_eq!(accounts.len(), 0);

    // Category list should be empty
    let categories = cat_service.list_categories(user_id).await.unwrap();
    assert_eq!(categories.len(), 0);

    // Can recreate account with same name "Itaú"
    let re_acc = acc_service
        .create_account(
            user_id,
            "Itaú".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(200.00)).unwrap(),
        )
        .await;
    assert!(re_acc.is_ok());

    // Can recreate category with same name "Alimentação"
    let re_cat = cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await;
    assert!(re_cat.is_ok());
}

#[tokio::test]
async fn test_purge_older_than() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    acc_service
        .create_account(
            user_id,
            "Bradesco".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
        )
        .await
        .unwrap();

    cat_service
        .create_category(user_id, "Saúde".to_string(), TransactionKind::Expense, None)
        .await
        .unwrap();

    let tx1 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Bradesco".to_string(),
            category_query: "Saúde".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(50.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            description: "Remédio antigo".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    let tx2 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Bradesco".to_string(),
            category_query: "Saúde".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(30.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Remédio recente".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Soft delete both
    tx_service
        .delete_transaction(user_id, tx1.id)
        .await
        .unwrap();
    tx_service
        .delete_transaction(user_id, tx2.id)
        .await
        .unwrap();

    // Artificially update tx1 deleted_at to 40 days ago
    let old_date = Utc::now() - Duration::days(40);
    sqlx::query("UPDATE transactions SET deleted_at = $1 WHERE id = $2")
        .bind(old_date)
        .bind(tx1.id.as_uuid())
        .execute(pool)
        .await
        .unwrap();

    // Purge records older than 30 days
    let summary = tx_service.purge_deleted(user_id, "30d").await.unwrap();
    assert_eq!(summary.purged_transactions, 1);

    // Verify tx1 was permanently removed from database
    let tx1_check: Option<domain::Transaction> =
        storage::TransactionRepository::find_by_id_including_deleted(pool, user_id, tx1.id)
            .await
            .unwrap();
    assert!(tx1_check.is_none());

    // Verify tx2 is still in deleted list (not purged)
    let tx2_check: Option<domain::Transaction> =
        storage::TransactionRepository::find_by_id_including_deleted(pool, user_id, tx2.id)
            .await
            .unwrap();
    assert!(tx2_check.is_some());
}
