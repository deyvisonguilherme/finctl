use storage::TestDb;

#[tokio::test]
async fn test_migrations_and_isolation_with_test_db() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id.as_uuid();

    // 1. Insert valid account
    let account_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, user_id, name, kind, initial_balance) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(account_id)
    .bind(user_id)
    .bind("Conta Corrente")
    .bind("checking")
    .bind(rust_decimal::Decimal::new(10000, 2)) // 100.00
    .execute(pool)
    .await
    .expect("insert valid account");

    // 2. Test duplicate account name for same user fails
    let dup_res = sqlx::query(
        "INSERT INTO accounts (id, user_id, name, kind, initial_balance) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(uuid::Uuid::new_v4())
    .bind(user_id)
    .bind("Conta Corrente")
    .bind("savings")
    .bind(rust_decimal::Decimal::ZERO)
    .execute(pool)
    .await;
    assert!(dup_res.is_err(), "Duplicate account name must fail");

    // 3. Test invalid account kind fails
    let invalid_kind_res = sqlx::query(
        "INSERT INTO accounts (id, user_id, name, kind, initial_balance) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(uuid::Uuid::new_v4())
    .bind(user_id)
    .bind("Conta Invalida")
    .bind("not_a_kind")
    .bind(rust_decimal::Decimal::ZERO)
    .execute(pool)
    .await;
    assert!(
        invalid_kind_res.is_err(),
        "Invalid account kind must fail constraint"
    );

    // 4. Insert category and test transactions
    let cat_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO categories (id, user_id, name, kind) VALUES ($1, $2, $3, $4)")
        .bind(cat_id)
        .bind(user_id)
        .bind("Alimentação")
        .bind("expense")
        .execute(pool)
        .await
        .expect("insert valid category");

    // 5. Test amount > 0 constraint on transactions
    let invalid_tx_res = sqlx::query(
        "INSERT INTO transactions (id, user_id, account_id, category_id, kind, amount, date, description) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    )
    .bind(uuid::Uuid::new_v4())
    .bind(user_id)
    .bind(account_id)
    .bind(cat_id)
    .bind("expense")
    .bind(rust_decimal::Decimal::ZERO)
    .bind(chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap())
    .bind("Teste")
    .execute(pool)
    .await;
    assert!(
        invalid_tx_res.is_err(),
        "Amount <= 0 must fail check constraint"
    );
}
