use app::{
    AccountService, CategoryService, CreateTransactionInput, ListTransactionsInput,
    TransactionService,
};
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_transaction_pagination_search_and_batch_actions() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    let _checking = acc_service
        .create_account(
            user_id,
            "Conta Corrente".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    let _category = cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 1. Criar 35 transações
    let mut created_ids = Vec::new();
    for i in 1..=35 {
        let desc = if i == 10 {
            "Mercado Especial Exclusivo".to_string()
        } else if i == 20 {
            "Outro Mercado Especial".to_string()
        } else {
            format!("Despesa de Teste #{:02}", i)
        };

        let status = if i % 2 == 0 {
            TransactionStatus::Pending
        } else {
            TransactionStatus::Paid
        };

        let today = chrono::Local::now().date_naive();
        let date = today
            .checked_sub_days(chrono::Days::new((i % 25) as u64))
            .unwrap();

        let tx = tx_service
            .create_transaction(CreateTransactionInput {
                user_id,
                account_query: "Conta Corrente".to_string(),
                category_query: "Alimentação".to_string(),
                kind: TransactionKind::Expense,
                amount: Money::from_decimal_non_negative(dec!(10.00)).unwrap(),
                date,
                description: desc,
                status: Some(status),
            })
            .await
            .unwrap();

        created_ids.push(tx.id);
    }

    // 2. Testar paginação: Página 1 (15 itens)
    let p1 = tx_service
        .list_transactions_paginated(ListTransactionsInput {
            user_id,
            from_date: None,
            to_date: None,
            month: None,
            account_query: None,
            category_query: None,
            kind: None,
            status: None,
            installment_group_id: None,
            tag: None,
            limit: Some(15),
            offset: Some(0),
            search_description: None,
            deleted: Some(false),
            all_time: false,
        })
        .await
        .unwrap();

    assert_eq!(p1.items.len(), 15);
    assert_eq!(p1.total_count, 35);
    assert_eq!(p1.page, 1);
    assert_eq!(p1.page_size, 15);
    assert_eq!(p1.total_pages, 3);

    // 3. Testar paginação: Página 2 (15 itens)
    let p2 = tx_service
        .list_transactions_paginated(ListTransactionsInput {
            user_id,
            from_date: None,
            to_date: None,
            month: None,
            account_query: None,
            category_query: None,
            kind: None,
            status: None,
            installment_group_id: None,
            tag: None,
            limit: Some(15),
            offset: Some(15),
            search_description: None,
            deleted: Some(false),
            all_time: false,
        })
        .await
        .unwrap();

    assert_eq!(p2.items.len(), 15);
    assert_eq!(p2.total_count, 35);
    assert_eq!(p2.page, 2);

    // 4. Testar paginação: Página 3 (5 itens restantes)
    let p3 = tx_service
        .list_transactions_paginated(ListTransactionsInput {
            user_id,
            from_date: None,
            to_date: None,
            month: None,
            account_query: None,
            category_query: None,
            kind: None,
            status: None,
            installment_group_id: None,
            tag: None,
            limit: Some(15),
            offset: Some(30),
            search_description: None,
            deleted: Some(false),
            all_time: false,
        })
        .await
        .unwrap();

    assert_eq!(p3.items.len(), 5);
    assert_eq!(p3.total_count, 35);
    assert_eq!(p3.page, 3);

    // 5. Testar busca por descrição parcial (ILIKE)
    let search_res = tx_service
        .list_transactions_paginated(ListTransactionsInput {
            user_id,
            from_date: None,
            to_date: None,
            month: None,
            account_query: None,
            category_query: None,
            kind: None,
            status: None,
            installment_group_id: None,
            tag: None,
            limit: Some(15),
            offset: Some(0),
            search_description: Some("mercado especial".to_string()),
            deleted: Some(false),
            all_time: false,
        })
        .await
        .unwrap();

    assert_eq!(search_res.total_count, 2);
    assert_eq!(search_res.items.len(), 2);
    assert!(search_res
        .items
        .iter()
        .any(|t| t.description.contains("Exclusivo")));

    // 6. Testar pagamento em lote (pay_multiple_transactions)
    let pending_ids = vec![created_ids[1], created_ids[3], created_ids[5]]; // pares (indices 1, 3, 5 são pendentes)
    let count_paid = tx_service
        .pay_multiple_transactions(user_id, &pending_ids, None)
        .await
        .unwrap();
    assert_eq!(count_paid, 3);

    // 7. Testar exclusão em lote (delete_multiple_transactions)
    let to_delete = vec![created_ids[0], created_ids[1]];
    let count_deleted = tx_service
        .delete_multiple_transactions(user_id, &to_delete)
        .await
        .unwrap();
    assert_eq!(count_deleted, 2);

    // Verificar se total_count agora reflete a exclusão
    let after_delete = tx_service
        .list_transactions_paginated(ListTransactionsInput {
            user_id,
            from_date: None,
            to_date: None,
            month: None,
            account_query: None,
            category_query: None,
            kind: None,
            status: None,
            installment_group_id: None,
            tag: None,
            limit: Some(15),
            offset: Some(0),
            search_description: None,
            deleted: Some(false),
            all_time: false,
        })
        .await
        .unwrap();

    assert_eq!(after_delete.total_count, 33);
}
