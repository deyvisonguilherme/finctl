use app::CategoryService;
use storage::TestDb;

#[tokio::test]
async fn test_seed_default_categories_idempotency() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let service = CategoryService::new(pool);

    // 1. Initial seed
    let created_first = service
        .seed_default_categories(user_id)
        .await
        .expect("primeiro seed");
    assert!(created_first > 0, "Deve criar categorias no primeiro seed");

    let list_first = service.list_categories(user_id).await.expect("listar");
    assert_eq!(list_first.len(), created_first);

    // 2. Second seed (idempotent)
    let created_second = service
        .seed_default_categories(user_id)
        .await
        .expect("segundo seed");
    assert_eq!(created_second, 0, "Segundo seed não deve duplicar");

    let list_second = service.list_categories(user_id).await.expect("listar");
    assert_eq!(list_second.len(), list_first.len());
}
