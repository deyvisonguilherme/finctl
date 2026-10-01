use storage::TestDb;

#[tokio::test]
async fn test_database_isolation_instance_1() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id.as_uuid();

    let account_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, user_id, name, kind, initial_balance) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(account_id)
    .bind(user_id)
    .bind("Conta Isolada 1")
    .bind("checking")
    .bind(rust_decimal::Decimal::ZERO)
    .execute(pool)
    .await
    .expect("insert account in test 1");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count accounts");

    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn test_database_isolation_instance_2() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id.as_uuid();

    // Verify there are no accounts for this fresh user/database
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count accounts");

    assert_eq!(count.0, 0);
}
