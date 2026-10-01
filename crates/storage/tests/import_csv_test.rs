use app::{
    AccountService, ImportCsvInput, ImportService, ListTransactionsInput, TransactionService,
};
use domain::{AccountKind, Money};
use rust_decimal_macros::dec;
use std::fs;
use storage::TestDb;

#[tokio::test]
async fn test_import_csv_workflow_and_idempotency() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let tx_service = TransactionService::new(pool);
    let import_service = ImportService::new(pool);

    // 1. Setup account
    acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    // 2. Prepare test CSV
    let tmp_path = format!("/tmp/test_import_{}.csv", uuid::Uuid::new_v4());
    let csv_content = "\
date,description,amount,category
2026-10-01,Salário Empresa,5000.00,Salário
2026-10-05,Supermercado Compras,-250.50,Alimentação
2026-10-10,Farmácia Saúde,-80.00,Saúde
2026-10-12,Compra Desconhecida,-15.00,
linha_invalida_sem_formato_correto
";
    fs::write(&tmp_path, csv_content).unwrap();

    // 3. Test dry-run first
    let dry_run_summary = import_service
        .import_csv(ImportCsvInput {
            user_id,
            account_query: "Nubank".to_string(),
            file_path: tmp_path.clone(),
            profile_name: None,
            dry_run: true,
        })
        .await
        .unwrap();

    assert!(dry_run_summary.dry_run);
    assert_eq!(dry_run_summary.total_rows, 5);
    assert_eq!(dry_run_summary.imported_count, 4);
    assert_eq!(dry_run_summary.duplicates_count, 0);
    assert_eq!(dry_run_summary.errors.len(), 1);
    assert_eq!(dry_run_summary.errors[0].line_number, 6);

    // Confirm nothing was persisted during dry-run
    let list_after_dry = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list_after_dry.len(), 0);

    // 4. Test actual import
    let real_summary_1 = import_service
        .import_csv(ImportCsvInput {
            user_id,
            account_query: "Nubank".to_string(),
            file_path: tmp_path.clone(),
            profile_name: None,
            dry_run: false,
        })
        .await
        .unwrap();

    assert!(!real_summary_1.dry_run);
    assert_eq!(real_summary_1.total_rows, 5);
    assert_eq!(real_summary_1.imported_count, 4);
    assert_eq!(real_summary_1.duplicates_count, 0);
    assert_eq!(real_summary_1.errors.len(), 1);

    // Verify persisted transactions
    let list = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list.len(), 4);

    // Verify unclassified item has category "A classificar"
    let unclassified = list
        .iter()
        .find(|t| t.description == "Compra Desconhecida")
        .expect("Deve encontrar a compra desconhecida");
    assert_eq!(unclassified.category_name, "A classificar");

    // 5. Test idempotency: re-running the exact same file
    let real_summary_2 = import_service
        .import_csv(ImportCsvInput {
            user_id,
            account_query: "Nubank".to_string(),
            file_path: tmp_path.clone(),
            profile_name: None,
            dry_run: false,
        })
        .await
        .unwrap();

    assert_eq!(real_summary_2.imported_count, 0);
    assert_eq!(real_summary_2.duplicates_count, 4);
    assert_eq!(real_summary_2.errors.len(), 1);

    // Verify count in db is still 4
    let list_after_reimport = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list_after_reimport.len(), 4);

    let _ = fs::remove_file(&tmp_path);
}
