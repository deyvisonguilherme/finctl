use app::CategoryService;
use domain::TransactionKind;
use storage::TestDb;

#[tokio::test]
async fn test_category_service_and_hierarchy() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;
    let service = CategoryService::new(pool);

    // 1. Create root category "Alimentação"
    let parent = service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .expect("criar categoria pai");
    assert_eq!(parent.name, "Alimentação");
    assert_eq!(parent.kind, TransactionKind::Expense);

    // 2. Create subcategory "Mercado"
    let child = service
        .create_category(
            user_id,
            "Mercado".to_string(),
            TransactionKind::Expense,
            Some("Alimentação"),
        )
        .await
        .expect("criar subcategoria");
    assert_eq!(child.name, "Mercado");
    assert_eq!(child.parent_id, Some(parent.id));

    // 3. Test duplicate subcategory under same parent fails
    let dup_res = service
        .create_category(
            user_id,
            "Mercado".to_string(),
            TransactionKind::Expense,
            Some("Alimentação"),
        )
        .await;
    assert!(
        dup_res.is_err(),
        "Duplicate category name under same parent must fail"
    );

    // 4. Test child kind different from parent kind fails
    let diff_kind_res = service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            Some("Alimentação"),
        )
        .await;
    assert!(
        diff_kind_res.is_err(),
        "Category with kind different from parent must fail"
    );

    // 5. Test 3rd level nesting (child of child) fails
    let nested_res = service
        .create_category(
            user_id,
            "Hortifruti".to_string(),
            TransactionKind::Expense,
            Some("Mercado"),
        )
        .await;
    assert!(nested_res.is_err(), "3rd level nesting must fail");

    // 6. Test list categories
    let list = service
        .list_categories(user_id)
        .await
        .expect("listar categorias");
    assert_eq!(list.len(), 2);
    let child_item = list.iter().find(|c| c.name == "Mercado").unwrap();
    assert_eq!(child_item.parent_name.as_deref(), Some("Alimentação"));
}
