use app::{AccountService, CategoryService, CreateTransactionInput, TransactionService};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind};
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
