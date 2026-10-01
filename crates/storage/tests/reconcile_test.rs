use app::{
    AccountService, CategoryService, CreateAccountInput, CreateTransactionInput, ReconcileInput,
    ReconcileService, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use std::fs;
use storage::TestDb;

#[tokio::test]
async fn test_reconciliation_workflow() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let reconcile_service = ReconcileService::new(pool);

    // 1. Create account & categories
    acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Nubank Conta".to_string(),
            kind: AccountKind::Checking,
            initial_balance: Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
            credit_limit: None,
            closing_day: None,
            due_day: None,
        })
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

    // 2. Create existing transactions
    // tx1: Salary on May 10th
    let tx1 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank Conta".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(5000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
            description: "Salario Empresa ABC".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // tx2: Grocery on May 12th
    let tx2 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank Conta".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(150.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
            description: "Supermercado Pao de Acucar".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // tx3: Restaurant on May 20th (will remain pending / unmatched)
    let tx3 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank Conta".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(80.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            description: "Restaurante Almoco".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // 3. Create CSV extrato
    let tmp_path = format!("/tmp/test_reconcile_{}.csv", uuid::Uuid::new_v4());
    let csv_content = "\
date,description,amount
2026-05-11,Salario ABC,5000.00
2026-05-12,Pao de Acucar Super,-150.00
2026-05-15,Farmacia DrogaRaia,-45.00
";
    fs::write(&tmp_path, csv_content).unwrap();

    // 4. Initial status: all 3 tx are unreconciled
    let status_before = reconcile_service
        .status(user_id, Some("Nubank Conta".to_string()))
        .await
        .unwrap();
    assert_eq!(status_before.total_unreconciled_count, 3);

    // 5. Test Dry Run (apply = false)
    let analysis_dry_run = reconcile_service
        .reconcile(ReconcileInput {
            user_id,
            account_query: "Nubank Conta".to_string(),
            file_path: tmp_path.clone(),
            profile: None,
            max_days: Some(3),
            apply: false,
        })
        .await
        .unwrap();

    assert_eq!(analysis_dry_run.matched_pairs.len(), 2);
    assert_eq!(analysis_dry_run.unmatched_csv_rows.len(), 1);
    assert_eq!(analysis_dry_run.unmatched_db_transactions.len(), 1);
    assert_eq!(analysis_dry_run.reconciled_count, 0);

    // Verify matches details
    let matched_ids: Vec<_> = analysis_dry_run
        .matched_pairs
        .iter()
        .map(|m| m.transaction.id)
        .collect();
    assert!(matched_ids.contains(&tx1.id));
    assert!(matched_ids.contains(&tx2.id));

    // Unmatched tx is tx3
    assert_eq!(analysis_dry_run.unmatched_db_transactions[0].id, tx3.id);

    // Unmatched extrato is the pharmacy row
    assert_eq!(
        analysis_dry_run.unmatched_csv_rows[0].description,
        "Farmacia DrogaRaia"
    );

    // Status still shows 3 unreconciled since apply was false
    let status_mid = reconcile_service
        .status(user_id, Some("Nubank Conta".to_string()))
        .await
        .unwrap();
    assert_eq!(status_mid.total_unreconciled_count, 3);

    // 6. Test with apply = true
    let analysis_applied = reconcile_service
        .reconcile(ReconcileInput {
            user_id,
            account_query: "Nubank Conta".to_string(),
            file_path: tmp_path.clone(),
            profile: None,
            max_days: Some(3),
            apply: true,
        })
        .await
        .unwrap();

    assert_eq!(analysis_applied.matched_pairs.len(), 2);
    assert_eq!(analysis_applied.reconciled_count, 2);

    // Status now shows only 1 unreconciled (tx3)
    let status_after = reconcile_service
        .status(user_id, Some("Nubank Conta".to_string()))
        .await
        .unwrap();
    assert_eq!(status_after.total_unreconciled_count, 1);
    assert_eq!(status_after.transactions[0].id, tx3.id);

    // 7. Verify tolerance window: if max_days = Some(0), tx1 (date 10 vs 11) won't match
    let analysis_tol_0 = reconcile_service
        .reconcile(ReconcileInput {
            user_id,
            account_query: "Nubank Conta".to_string(),
            file_path: tmp_path.clone(),
            profile: None,
            max_days: Some(0),
            apply: false,
        })
        .await
        .unwrap();

    // Since tx2 is already reconciled in DB, with tolerance 0, tx1 won't match (1 day diff),
    // and tx2 is excluded because it's already reconciled!
    assert_eq!(analysis_tol_0.matched_pairs.len(), 0);

    let _ = fs::remove_file(tmp_path);
}
