use app::{
    AccountService, CategoryService, CreateTransactionInput, EditTransactionInput,
    ListTransactionsInput, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionId, TransactionKind};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_create_income_and_expense_transactions() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Setup account and categories
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
        .expect("criar categoria mercado");

    cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .expect("criar categoria salario");

    // 2. Add expense
    let expense_tx = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(89.90)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Compras da semana".to_string(),
        })
        .await
        .expect("registrar despesa");

    assert_eq!(expense_tx.amount.as_decimal(), dec!(89.90));
    assert_eq!(expense_tx.kind, TransactionKind::Expense);

    // 3. Add income
    let income_tx = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(5000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Salário mensal".to_string(),
        })
        .await
        .expect("registrar receita");

    assert_eq!(income_tx.amount.as_decimal(), dec!(5000.00));
    assert_eq!(income_tx.kind, TransactionKind::Income);

    // 4. Test invalid category kind (expense using income category)
    let invalid_res = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(50.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 6).unwrap(),
            description: "Invalido".to_string(),
        })
        .await;

    assert!(
        invalid_res.is_err(),
        "Expense com categoria de receita deve falhar"
    );
}

#[tokio::test]
async fn test_transaction_list_filters() {
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
            Money::ZERO,
        )
        .await
        .unwrap();

    acc_service
        .create_account(
            user_id,
            "Inter".to_string(),
            AccountKind::Checking,
            Money::ZERO,
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

    cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    // Insert multiple transactions across dates
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(50.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
            description: "Almoço Setembro".to_string(),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Inter".to_string(),
            category_query: "Alimentação".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(100.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(),
            description: "Jantar Outubro".to_string(),
        })
        .await
        .unwrap();

    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Salário".to_string(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(3000.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Salário Outubro".to_string(),
        })
        .await
        .unwrap();

    // Filter by month 2026-10
    let oct_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(oct_txs.len(), 2);

    // Filter by account "Nubank"
    let nubank_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            account_query: Some("Nubank".to_string()),
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(nubank_txs.len(), 1);
    assert_eq!(nubank_txs[0].description, "Salário Outubro");

    // Filter by kind Income
    let income_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            kind: Some(TransactionKind::Income),
            from_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            to_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(income_txs.len(), 1);

    // Limit test
    let limit_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            from_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            to_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            limit: Some(2),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(limit_txs.len(), 2);
}

#[tokio::test]
async fn test_transaction_edit_and_delete() {
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
            Money::ZERO,
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

    let tx = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Nubank".to_string(),
            category_query: "Mercado".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(50.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            description: "Original".to_string(),
        })
        .await
        .unwrap();

    // 1. Edit transaction
    let updated = tx_service
        .edit_transaction(EditTransactionInput {
            user_id,
            id: tx.id,
            account_query: None,
            category_query: None,
            amount: Some(Money::new(dec!(75.50)).unwrap()),
            date: Some(NaiveDate::from_ymd_opt(2026, 10, 2).unwrap()),
            description: Some("Modificado".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(updated.amount.as_decimal(), dec!(75.50));
    assert_eq!(updated.description, "Modificado");

    // 2. Delete transaction
    tx_service
        .delete_transaction(user_id, tx.id)
        .await
        .expect("deletar transação com sucesso");

    // 3. Trying to delete again returns not found
    let err = tx_service
        .delete_transaction(user_id, tx.id)
        .await
        .unwrap_err();
    assert!(matches!(err, app::AppError::NotFound(_)));

    // 4. Trying to edit non-existent returns not found
    let edit_err = tx_service
        .edit_transaction(EditTransactionInput {
            user_id,
            id: TransactionId::generate(),
            account_query: None,
            category_query: None,
            amount: None,
            date: None,
            description: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(edit_err, app::AppError::NotFound(_)));
}
