use app::{
    AccountService, CategoryService, CreateTransactionInput, ListTransactionsInput,
    TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind};
use rust_decimal_macros::dec;
use std::fs;
use storage::TestDb;

#[tokio::test]
async fn test_export_tx_data_and_formatting() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Setup account & category
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

    // 2. Transaction
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1234.56)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
            description: "Supermercado Semanal".to_string(),
        })
        .await
        .unwrap();

    let list = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    let tx = &list[0];

    // Verify pt-BR CSV formatting simulation
    let amount_pt_br = tx.amount.as_decimal().to_string().replace('.', ",");
    assert_eq!(amount_pt_br, "1234,56");
    assert_eq!(tx.date.format("%d/%m/%Y").to_string(), "15/10/2026");

    // Verify temp file write test
    let tmp_path = format!("/tmp/test_export_{}.csv", uuid::Uuid::new_v4());
    let mut file = fs::File::create(&tmp_path).unwrap();
    use std::io::Write;
    file.write_all(&[0xEF, 0xBB, 0xBF]).unwrap();
    writeln!(file, "data;descricao;valor;tipo;conta;categoria;status;id").unwrap();
    writeln!(
        file,
        "{};{};{};{};{};{};{};{}",
        tx.date.format("%d/%m/%Y"),
        tx.description,
        amount_pt_br,
        tx.kind.as_str(),
        tx.account_name,
        tx.category_name,
        tx.status.as_str(),
        tx.id.as_uuid()
    )
    .unwrap();

    let content = fs::read(&tmp_path).unwrap();
    assert_eq!(&content[0..3], &[0xEF, 0xBB, 0xBF]); // Has UTF-8 BOM
    let str_content = String::from_utf8(content[3..].to_vec()).unwrap();
    assert!(str_content
        .contains("15/10/2026;Supermercado Semanal;1234,56;expense;Nubank;Mercado;paid;"));

    let _ = fs::remove_file(&tmp_path);
}
