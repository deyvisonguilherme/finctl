use app::{
    AccountService, CategoryService, CreateRecurringInput, EditRecurringInput,
    ListTransactionsInput, RecurringService, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, RecurringFrequency, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_recurring_rules_crud_and_persistence() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let rec_service = RecurringService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Setup account and category
    let account = acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(3000.00)).unwrap(),
        )
        .await
        .unwrap();

    let category = cat_service
        .create_category(
            user_id,
            "Aluguel".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 2. Create monthly recurring rule: Aluguel R$ 1.500,00 todo dia 5
    let rule = rec_service
        .create_rule(CreateRecurringInput {
            user_id,
            account_query: account.name.clone(),
            category_query: category.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1500.00)).unwrap(),
            description: "Aluguel Apartamento".to_string(),
            frequency: RecurringFrequency::Monthly,
            day_of_month: Some(5),
            day_of_week: None,
            start_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            end_date: None,
        })
        .await
        .unwrap();

    assert_eq!(rule.frequency, RecurringFrequency::Monthly);
    assert_eq!(rule.day_of_month, Some(5));
    assert!(rule.active);

    // 3. List recurring rules
    let list_active = rec_service.list_rules(user_id, Some(true)).await.unwrap();
    assert_eq!(list_active.len(), 1);
    assert_eq!(list_active[0].description, "Aluguel Apartamento");
    assert_eq!(list_active[0].account_name, "Nubank");
    assert_eq!(list_active[0].category_name, "Aluguel");

    // 4. Pause and Resume rule
    let paused = rec_service.pause_rule(user_id, rule.id).await.unwrap();
    assert!(!paused.active);

    let list_active_after_pause = rec_service.list_rules(user_id, Some(true)).await.unwrap();
    assert_eq!(list_active_after_pause.len(), 0);

    let list_all_after_pause = rec_service.list_rules(user_id, None).await.unwrap();
    assert_eq!(list_all_after_pause.len(), 1);

    let resumed = rec_service.resume_rule(user_id, rule.id).await.unwrap();
    assert!(resumed.active);

    // 5. Edit rule (update amount and description)
    let edited = rec_service
        .edit_rule(EditRecurringInput {
            user_id,
            id: rule.id,
            account_query: None,
            category_query: None,
            amount: Some(Money::new(dec!(1650.00)).unwrap()),
            description: Some("Aluguel Apartamento com Reajuste".to_string()),
            end_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
        })
        .await
        .unwrap();

    assert_eq!(edited.amount, Money::new(dec!(1650.00)).unwrap());
    assert_eq!(edited.description, "Aluguel Apartamento com Reajuste");
    assert_eq!(
        edited.end_date,
        Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap())
    );

    // 6. Test that removing a recurring rule does NOT delete existing transactions
    // Create a transaction linked to the recurring_rule_id
    let tx = domain::Transaction::new_full(
        user_id,
        account.id,
        category.id,
        TransactionKind::Expense,
        Money::new(dec!(1500.00)).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        "Aluguel Outubro".to_string(),
        TransactionStatus::Paid,
        None,
        None,
        None,
        None,
        Some(rule.id.as_uuid()),
        None,
        None,
    )
    .unwrap();
    storage::TransactionRepository::create(pool, &tx)
        .await
        .unwrap();

    // Remove the rule
    rec_service.delete_rule(user_id, rule.id).await.unwrap();

    // Verify rule is deleted
    let remaining_rules = rec_service.list_rules(user_id, None).await.unwrap();
    assert_eq!(remaining_rules.len(), 0);

    // Verify transaction is still intact
    let txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].description, "Aluguel Outubro");
}
