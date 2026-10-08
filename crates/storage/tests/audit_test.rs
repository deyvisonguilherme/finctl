use app::{
    AccountService, AuditFilter, AuditService, BudgetService, CategoryService,
    CreateTransactionInput, EditTransactionInput, ImportCsvInput, ImportService,
    TransactionService,
};
use domain::{AccountKind, AuditAction, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use std::fs;
use std::time::Instant;
use storage::{BudgetRepository, TestDb};

#[tokio::test]
async fn test_audit_log_captures_all_mutations_and_actor() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    // Configurar ator de teste no ambiente
    std::env::set_var("FINCTL_ACTOR", "test-agent-deyvison");

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let budget_service = BudgetService::new(pool);
    let audit_service = AuditService::new(pool);

    // 1. Criar Conta -> INSERT em accounts
    let account = acc_service
        .create_account(
            user_id,
            "Conta Auditoria".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
        )
        .await
        .unwrap();

    let account_audits = audit_service
        .list(AuditFilter {
            table_name: Some("accounts".to_string()),
            row_id: Some(account.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(account_audits.len(), 1);
    assert_eq!(account_audits[0].action, AuditAction::Insert);
    assert_eq!(account_audits[0].actor, "test-agent-deyvison");
    assert!(account_audits[0].old.is_none());
    assert!(account_audits[0].new.is_some());
    assert_eq!(
        account_audits[0].new.as_ref().unwrap()["name"],
        "Conta Auditoria"
    );

    // 2. Criar Categoria -> INSERT em categories
    let category = cat_service
        .create_category(
            user_id,
            "Alimentação Audit".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_audits = audit_service
        .list(AuditFilter {
            table_name: Some("categories".to_string()),
            row_id: Some(category.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(cat_audits.len(), 1);
    assert_eq!(cat_audits[0].action, AuditAction::Insert);
    assert_eq!(cat_audits[0].actor, "test-agent-deyvison");

    // 3. Criar Transação -> INSERT em transactions
    let tx = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: category.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(75.50)).unwrap(),
            date: chrono::NaiveDate::from_ymd_opt(2026, 10, 7).unwrap(),
            description: "Almoço Teste".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    let tx_audits = audit_service
        .list(AuditFilter {
            table_name: Some("transactions".to_string()),
            row_id: Some(tx.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(tx_audits.len(), 1);
    assert_eq!(tx_audits[0].action, AuditAction::Insert);
    assert_eq!(tx_audits[0].actor, "test-agent-deyvison");

    // 4. Editar Transação -> UPDATE em transactions
    tx_service
        .edit_transaction(EditTransactionInput {
            user_id,
            id: tx.id,
            account_query: None,
            category_query: None,
            amount: Some(Money::from_decimal_non_negative(dec!(99.00)).unwrap()),
            date: None,
            description: Some("Almoço Especial".to_string()),
        })
        .await
        .unwrap();

    let tx_audits = audit_service
        .list(AuditFilter {
            table_name: Some("transactions".to_string()),
            row_id: Some(tx.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(tx_audits.len(), 2);
    // Ordenado por changed_at desc: o mais recente é UPDATE
    assert_eq!(tx_audits[0].action, AuditAction::Update);
    assert_eq!(tx_audits[0].actor, "test-agent-deyvison");
    assert!(tx_audits[0].old.is_some());
    assert!(tx_audits[0].new.is_some());

    // 5. Soft Delete -> UPDATE com deleted_at preenchido
    tx_service.delete_transaction(user_id, tx.id).await.unwrap();

    let tx_audits = audit_service
        .list(AuditFilter {
            table_name: Some("transactions".to_string()),
            row_id: Some(tx.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(tx_audits.len(), 3);
    assert_eq!(tx_audits[0].action, AuditAction::Update);
    assert_eq!(tx_audits[0].diff_summary(), "soft delete (removido)");

    // 6. Restore -> UPDATE com deleted_at limpo (null)
    tx_service
        .restore_transaction(user_id, tx.id, false)
        .await
        .unwrap();

    let tx_audits = audit_service
        .list(AuditFilter {
            table_name: Some("transactions".to_string()),
            row_id: Some(tx.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(tx_audits.len(), 4);
    assert_eq!(tx_audits[0].action, AuditAction::Update);
    assert_eq!(tx_audits[0].diff_summary(), "restaurado");

    // 7. Orçamento -> INSERT e DELETE em budgets
    let budget = budget_service
        .set_budget(
            user_id,
            category.name.clone(),
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
            None,
        )
        .await
        .unwrap();

    let b_audits = audit_service
        .list(AuditFilter {
            table_name: Some("budgets".to_string()),
            row_id: Some(budget.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(b_audits.len(), 1);
    assert_eq!(b_audits[0].action, AuditAction::Insert);

    BudgetRepository::delete(pool, user_id, budget.id)
        .await
        .unwrap();

    let b_audits = audit_service
        .list(AuditFilter {
            table_name: Some("budgets".to_string()),
            row_id: Some(budget.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(b_audits.len(), 2);
    assert_eq!(b_audits[0].action, AuditAction::Delete);
}

#[tokio::test]
async fn test_audit_log_immutability() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    acc_service
        .create_account(
            user_id,
            "Conta Imutavel".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(100.00)).unwrap(),
        )
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(count >= 1);

    // Tentativa de UPDATE em audit_log deve falhar
    let update_res = sqlx::query("UPDATE audit_log SET actor = 'fraude'")
        .execute(pool)
        .await;

    assert!(update_res.is_err());
    let err_msg = update_res.unwrap_err().to_string();
    assert!(
        err_msg.contains("append-only"),
        "Erro esperado de imutabilidade, obtido: {err_msg}"
    );

    // Tentativa de DELETE em audit_log deve falhar
    let delete_res = sqlx::query("DELETE FROM audit_log").execute(pool).await;

    assert!(delete_res.is_err());
    let err_msg = delete_res.unwrap_err().to_string();
    assert!(
        err_msg.contains("append-only"),
        "Erro esperado de imutabilidade, obtido: {err_msg}"
    );
}

#[tokio::test]
async fn test_audit_log_filter_queries() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let audit_service = AuditService::new(pool);

    let acc1 = acc_service
        .create_account(
            user_id,
            "Conta Filtro 1".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(10.00)).unwrap(),
        )
        .await
        .unwrap();

    let acc2 = acc_service
        .create_account(
            user_id,
            "Conta Filtro 2".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(20.00)).unwrap(),
        )
        .await
        .unwrap();

    // Filtro por ID
    let audits_acc1 = audit_service
        .list(AuditFilter {
            row_id: Some(acc1.id.as_uuid()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(audits_acc1.len(), 1);
    assert_eq!(audits_acc1[0].row_id, acc1.id.as_uuid());

    // Filtro por Limit
    let audits_limit = audit_service
        .list(AuditFilter {
            table_name: Some("accounts".to_string()),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(audits_limit.len(), 1);
    assert_eq!(audits_limit[0].row_id, acc2.id.as_uuid());
}

#[tokio::test]
async fn test_1000_rows_import_performance_with_audit() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let import_service = ImportService::new(pool);

    acc_service
        .create_account(
            user_id,
            "Conta Benchmark".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(10000.00)).unwrap(),
        )
        .await
        .unwrap();

    // Gerar CSV com 1.000 linhas
    let mut csv_data = String::from("date,description,amount,category\n");
    for i in 1..=1000 {
        let amount = (i as f64) * 1.5;
        csv_data.push_str(&format!(
            "2026-10-01,Transação Importada #{i},{amount:.2},Despesas Gerais\n"
        ));
    }

    let tmp_path = format!("/tmp/test_audit_import_{}.csv", uuid::Uuid::new_v4());
    fs::write(&tmp_path, csv_data).unwrap();

    // 1. Benchmark SEM triggers (desabilitando temporariamente na tabela transactions)
    sqlx::query("ALTER TABLE transactions DISABLE TRIGGER trg_audit_transactions")
        .execute(pool)
        .await
        .unwrap();

    let start_before = Instant::now();
    let summary_before = import_service
        .import_csv(ImportCsvInput {
            user_id,
            account_query: "Conta Benchmark".to_string(),
            file_path: tmp_path.clone(),
            profile_name: None,
            dry_run: false,
        })
        .await
        .unwrap();
    let duration_before = start_before.elapsed();
    assert_eq!(summary_before.imported_count, 1000);

    // Limpar transações para o segundo teste
    sqlx::query("DELETE FROM transactions WHERE user_id = $1")
        .bind(user_id.as_uuid())
        .execute(pool)
        .await
        .unwrap();

    // 2. Reativar o trigger de auditoria
    sqlx::query("ALTER TABLE transactions ENABLE TRIGGER trg_audit_transactions")
        .execute(pool)
        .await
        .unwrap();

    // Limpar tabela audit_log
    sqlx::query("TRUNCATE audit_log")
        .execute(pool)
        .await
        .unwrap_err(); // O truncate é bloqueado pela trigger de imutabilidade, então usamos outra conta

    acc_service
        .create_account(
            user_id,
            "Conta Benchmark Com Triggers".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(10000.00)).unwrap(),
        )
        .await
        .unwrap();

    let start_after = Instant::now();
    let summary_after = import_service
        .import_csv(ImportCsvInput {
            user_id,
            account_query: "Conta Benchmark Com Triggers".to_string(),
            file_path: tmp_path.clone(),
            profile_name: None,
            dry_run: false,
        })
        .await
        .unwrap();
    let duration_after = start_after.elapsed();

    let _ = fs::remove_file(tmp_path);

    assert_eq!(summary_after.imported_count, 1000);
    assert_eq!(summary_after.errors.len(), 0);

    // Verificar que foram criados 1.000 registros na tabela audit_log para transactions
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE table_name = 'transactions'")
            .fetch_one(pool)
            .await
            .unwrap();

    assert_eq!(audit_count, 1000);

    println!(
        "\n========================================\n\
         BENCHMARK DE IMPORTAÇÃO (1.000 LINHAS):\n\
         - SEM triggers: {:.2?}\n\
         - COM triggers: {:.2?}\n\
         ========================================",
        duration_before, duration_after
    );

    // Ambos os tempos devem ser aceitáveis (menos de 5 segundos)
    assert!(
        duration_after.as_secs() < 5,
        "Importação de 1000 linhas levou tempo excessivo: {:?}",
        duration_after
    );
}
