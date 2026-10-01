use domain::{Account, AccountKind, Money};
use rust_decimal_macros::dec;
use storage::{AccountRepository, StorageError, TestDb};

#[tokio::test]
async fn test_account_creation_and_listing() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let balance = Money::from_decimal_non_negative(dec!(1500.00)).unwrap();
    let account = Account::new(
        user_id,
        "Nubank".to_string(),
        AccountKind::Checking,
        balance,
    )
    .unwrap();

    AccountRepository::create(pool, &account)
        .await
        .expect("criar conta nubank");

    let accounts = AccountRepository::list_by_user(pool, user_id)
        .await
        .expect("listar contas");

    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "Nubank");
    assert_eq!(accounts[0].kind, AccountKind::Checking);
    assert_eq!(accounts[0].initial_balance.as_decimal(), dec!(1500.00));

    // Test duplicate account name returns UniqueViolation
    let duplicate = Account::new(
        user_id,
        "Nubank".to_string(),
        AccountKind::Savings,
        Money::ZERO,
    )
    .unwrap();
    let err = AccountRepository::create(pool, &duplicate)
        .await
        .unwrap_err();
    assert!(matches!(err, StorageError::UniqueViolation(_)));

    // Test find by name (case-insensitive)
    let found = AccountRepository::find_by_id_or_name(pool, user_id, "nubank")
        .await
        .expect("find by name")
        .expect("account exists");
    assert_eq!(found.id, account.id);

    // Test find by ID
    let found_by_id = AccountRepository::find_by_id_or_name(pool, user_id, &account.id.to_string())
        .await
        .expect("find by id")
        .expect("account exists");
    assert_eq!(found_by_id.name, "Nubank");
}
