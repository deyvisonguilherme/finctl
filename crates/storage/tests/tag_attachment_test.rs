use app::{
    AccountService, AttachmentService, CategoryReportInput, CategoryService, CreateAccountInput,
    CreateTransactionInput, ListTransactionsInput, ReportService, TagService, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use std::fs;
use storage::TestDb;

#[tokio::test]
async fn test_tags_and_attachments_workflow() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let tag_service = TagService::new(pool);
    let attachment_service = AttachmentService::new(pool);
    let report_service = ReportService::new(pool);

    // 1. Setup account and categories
    let acc = acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Conta Principal".to_string(),
            kind: AccountKind::Checking,
            initial_balance: Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
            credit_limit: None,
            closing_day: None,
            due_day: None,
        })
        .await
        .unwrap();

    let cat_travel = cat_service
        .create_category(
            user_id,
            "Viagens".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_home = cat_service
        .create_category(
            user_id,
            "Moradia".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 2. Create transactions
    let tx1 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc.name.clone(),
            category_query: cat_travel.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(450.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Passagem Aérea".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    let tx2 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc.name.clone(),
            category_query: cat_home.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
            description: "Tintas para Reforma".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    let _tx3 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc.name.clone(),
            category_query: cat_travel.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(120.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            description: "Almoço na Viagem".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // 3. Create and associate tags
    let tag_viagem = tag_service
        .add_tag(user_id, "ferias-2026".to_string())
        .await
        .unwrap();
    assert_eq!(tag_viagem.name, "ferias-2026");

    let tags_tx1 = tag_service
        .tag_transaction(
            user_id,
            &tx1.id.to_string(),
            vec!["ferias-2026".to_string(), "trabalho".to_string()],
        )
        .await
        .unwrap();
    assert_eq!(tags_tx1.len(), 2);

    let tags_tx2 = tag_service
        .tag_transaction(user_id, &tx2.id.to_string(), vec!["reforma".to_string()])
        .await
        .unwrap();
    assert_eq!(tags_tx2.len(), 1);

    // 4. List tags and verify usage count
    let tags_list = tag_service.list_tags(user_id).await.unwrap();
    assert_eq!(tags_list.len(), 3);
    let tag_ferias = tags_list.iter().find(|t| t.name == "ferias-2026").unwrap();
    assert_eq!(tag_ferias.usage_count, 1);

    // 5. Query transactions filtered by tag
    let list_ferias = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            tag: Some("ferias-2026".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list_ferias.len(), 1);
    assert_eq!(list_ferias[0].id, tx1.id);
    assert!(list_ferias[0].tags.contains(&"ferias-2026".to_string()));
    assert!(list_ferias[0].tags.contains(&"trabalho".to_string()));

    let list_reforma = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            tag: Some("reforma".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list_reforma.len(), 1);
    assert_eq!(list_reforma[0].id, tx2.id);

    // 6. Category report filtered by tag
    let cat_report_all = report_service
        .category_report(CategoryReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    // Total spent across all 3 transactions: 450 + 200 + 120 = 770
    assert_eq!(cat_report_all.total_amount.as_decimal(), dec!(770.00));

    let cat_report_tagged = report_service
        .category_report(CategoryReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            tag: Some("ferias-2026".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    // Only tx1 is tagged with ferias-2026 -> 450.00
    assert_eq!(cat_report_tagged.total_amount.as_decimal(), dec!(450.00));
    assert_eq!(cat_report_tagged.items.len(), 1);
    assert_eq!(cat_report_tagged.items[0].category_name, "Viagens");

    // 7. Attachments: Local file with SHA-256 calculation
    let tmp_file = format!("/tmp/comprovante_{}.pdf", uuid::Uuid::new_v4());
    fs::write(&tmp_file, b"CONTEUDO_DO_COMPROVANTE_12345").unwrap();

    let att1 = attachment_service
        .attach(
            user_id,
            &tx1.id.to_string(),
            &tmp_file,
            Some("Comprovante da passagem".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(att1.transaction_id, tx1.id);
    assert_eq!(att1.uri, tmp_file);
    assert!(att1.sha256.is_some());
    assert_eq!(att1.note.as_deref(), Some("Comprovante da passagem"));

    // 8. Attachments: Non-existent local file returns error
    let non_existent = format!("/tmp/nao_existe_{}.pdf", uuid::Uuid::new_v4());
    let err_att = attachment_service
        .attach(user_id, &tx1.id.to_string(), &non_existent, None)
        .await;
    assert!(err_att.is_err());
    assert!(format!("{err_att:?}").contains("não encontrado"));

    // 9. Attachments: Remote URL without local file check
    let att_url = attachment_service
        .attach(
            user_id,
            &tx1.id.to_string(),
            "https://banco.com.br/comprovantes/12345.pdf",
            Some("Link do banco".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(att_url.uri, "https://banco.com.br/comprovantes/12345.pdf");
    assert!(att_url.sha256.is_none());

    // 10. List attachments for transaction
    let atts = attachment_service
        .list_attachments(user_id, &tx1.id.to_string())
        .await
        .unwrap();
    assert_eq!(atts.len(), 2);

    // 11. Delete tag with safety check
    let del_err = tag_service.delete_tag(user_id, "ferias-2026", false).await;
    assert!(del_err.is_err());
    assert!(format!("{del_err:?}").contains("está associada a 1 lançamento(s)"));

    let del_ok = tag_service
        .delete_tag(user_id, "ferias-2026", true)
        .await
        .unwrap();
    assert!(del_ok.deleted);

    // Clean up temp file
    let _ = fs::remove_file(tmp_file);
}
