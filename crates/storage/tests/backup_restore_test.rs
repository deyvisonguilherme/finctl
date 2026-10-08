use app::{
    AccountService, BackupInput, BackupService, BalanceService, CategoryService,
    CreateTransactionInput, RestoreInput, TransactionService,
};
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use std::fs;
use storage::TestDb;

#[tokio::test]
async fn test_backup_and_restore_workflow_full() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;
    let db_url = &test_db.database_url;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let bal_service = BalanceService::new(pool);
    let backup_service = BackupService::new(pool);

    // 1. Criar dados: conta, categoria e transação
    let acc = acc_service
        .create_account(
            user_id,
            "Banco do Brasil".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(2000.00)).unwrap(),
        )
        .await
        .unwrap();

    let cat = cat_service
        .create_category(
            user_id,
            "Serviços".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc.name.clone(),
            category_query: cat.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(350.00)).unwrap(),
            date: chrono::NaiveDate::from_ymd_opt(2026, 10, 8).unwrap(),
            description: "Conta de Energia".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Saldo antes do backup
    let balance_before = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(balance_before.total_balance, dec!(1650.00));

    // 2. Executar Backup
    let tmp_backup_dir =
        std::env::temp_dir().join(format!("finctl_test_bck_{}", uuid::Uuid::new_v4()));
    let backup_summary = backup_service
        .backup(BackupInput {
            database_url: db_url.clone(),
            output_dir: tmp_backup_dir.clone(),
            keep: None,
        })
        .await
        .unwrap();

    assert!(backup_summary.file_path.exists());
    assert!(backup_summary.file_size_bytes > 0);
    assert!(!backup_summary.file_path.to_str().unwrap().ends_with(".tmp"));

    // 3. Apagar dados do banco de dados (simular perda de dados)
    sqlx::query("DELETE FROM transactions WHERE user_id = $1")
        .bind(user_id.as_uuid())
        .execute(pool)
        .await
        .unwrap();

    sqlx::query("DELETE FROM accounts WHERE user_id = $1")
        .bind(user_id.as_uuid())
        .execute(pool)
        .await
        .unwrap();

    let balance_empty = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(balance_empty.accounts.len(), 0);

    // 4. Executar Restore sobre o banco atual (com yes: true)
    let conn_info = storage::DatabaseConnectionInfo::parse(db_url).unwrap();
    let restore_summary = backup_service
        .restore(RestoreInput {
            database_url: db_url.clone(),
            file_path: backup_summary.file_path.clone(),
            into_database: Some(conn_info.database.clone()),
            yes: true,
        })
        .await
        .unwrap();

    assert_eq!(restore_summary.target_database, conn_info.database);
    assert!(!restore_summary.is_new_database);

    // 5. Verificar que saldos e dados foram 100% restaurados
    let balance_after = bal_service.get_balance(user_id, None, false).await.unwrap();
    assert_eq!(balance_after.total_balance, balance_before.total_balance);
    assert_eq!(balance_after.accounts.len(), 1);
    assert_eq!(balance_after.accounts[0].account_name, "Banco do Brasil");

    let _ = fs::remove_dir_all(&tmp_backup_dir);
}

#[tokio::test]
async fn test_restore_into_new_database() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;
    let db_url = &test_db.database_url;

    let acc_service = AccountService::new(pool);
    let backup_service = BackupService::new(pool);

    acc_service
        .create_account(
            user_id,
            "Conta Nova Base".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    let tmp_backup_dir =
        std::env::temp_dir().join(format!("finctl_test_bck_{}", uuid::Uuid::new_v4()));
    let backup_summary = backup_service
        .backup(BackupInput {
            database_url: db_url.clone(),
            output_dir: tmp_backup_dir.clone(),
            keep: None,
        })
        .await
        .unwrap();

    // Restore em banco novo (into_database: None)
    let restore_summary = backup_service
        .restore(RestoreInput {
            database_url: db_url.clone(),
            file_path: backup_summary.file_path.clone(),
            into_database: None,
            yes: false,
        })
        .await
        .unwrap();

    assert!(restore_summary.is_new_database);
    assert!(restore_summary.new_database_url.is_some());

    let new_url = restore_summary.new_database_url.unwrap();
    let new_pool = storage::create_pool(&new_url).await.unwrap();
    let acc_service_new = AccountService::new(&new_pool);
    let accounts_new = acc_service_new.list_accounts(user_id).await.unwrap();

    assert_eq!(accounts_new.len(), 1);
    assert_eq!(accounts_new[0].name, "Conta Nova Base");

    let _ = fs::remove_dir_all(&tmp_backup_dir);
}

#[tokio::test]
async fn test_backup_retention_policy_keep_n() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let db_url = &test_db.database_url;
    let backup_service = BackupService::new(pool);

    let tmp_backup_dir =
        std::env::temp_dir().join(format!("finctl_test_rot_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp_backup_dir).unwrap();

    // Criar arquivos artificiais antigos no diretório
    fs::write(
        tmp_backup_dir.join("finctl_backup_20261001_100000.dump"),
        b"dummy1",
    )
    .unwrap();
    fs::write(
        tmp_backup_dir.join("finctl_backup_20261002_100000.dump"),
        b"dummy2",
    )
    .unwrap();
    fs::write(
        tmp_backup_dir.join("finctl_backup_20261003_100000.dump"),
        b"dummy3",
    )
    .unwrap();

    // Executar backup com keep: Some(2)
    let summary = backup_service
        .backup(BackupInput {
            database_url: db_url.clone(),
            output_dir: tmp_backup_dir.clone(),
            keep: Some(2),
        })
        .await
        .unwrap();

    assert_eq!(summary.rotated_files.len(), 2);

    let mut remaining = Vec::new();
    for entry in fs::read_dir(&tmp_backup_dir).unwrap().flatten() {
        remaining.push(entry.file_name().to_string_lossy().to_string());
    }

    assert_eq!(remaining.len(), 2);
    // O mais recente gerado hoje deve estar entre os remanescentes
    assert!(remaining
        .iter()
        .any(|f| f == summary.file_path.file_name().unwrap().to_str().unwrap()));

    let _ = fs::remove_dir_all(&tmp_backup_dir);
}

#[tokio::test]
async fn test_error_when_pg_dump_missing_from_path() {
    // Apontar FINCTL_PG_DUMP_PATH para caminho inexistente
    std::env::set_var("FINCTL_PG_DUMP_PATH", "/caminho/inexistente/pg_dump");
    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", "/caminho/vazio");

    let err = BackupService::find_pg_dump().unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("O utilitário 'pg_dump' não foi encontrado no PATH"));

    // Restaurar ambiente
    std::env::set_var("PATH", old_path);
    storage::setup_test_postgres_tools();
}

#[tokio::test]
async fn test_restore_over_current_db_requires_yes() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let db_url = &test_db.database_url;
    let backup_service = BackupService::new(pool);

    let conn_info = storage::DatabaseConnectionInfo::parse(db_url).unwrap();

    let dummy_file = std::env::temp_dir().join(format!("dummy_{}.dump", uuid::Uuid::new_v4()));
    fs::write(&dummy_file, b"dummy").unwrap();

    let err = backup_service
        .restore(RestoreInput {
            database_url: db_url.clone(),
            file_path: dummy_file.clone(),
            into_database: Some(conn_info.database),
            yes: false,
        })
        .await
        .unwrap_err();

    let err_str = err.to_string();
    assert!(err_str.contains("requer confirmação. Utilize a flag --yes"));

    let _ = fs::remove_file(dummy_file);
}
