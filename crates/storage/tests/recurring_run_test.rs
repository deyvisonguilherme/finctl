use app::{
    AccountService, CategoryService, CreateRecurringInput, ListTransactionsInput, RecurringService,
    RunRecurringInput, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, RecurringFrequency, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_recurring_run_full_cycle_idempotency_and_dry_run() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let rec_service = RecurringService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Setup accounts and categories
    let account = acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    let cat_rent = cat_service
        .create_category(
            user_id,
            "Moradia".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_salary = cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    // 2. Create monthly expense: Moradia R$ 2.000 todo dia 31 (start 2026-01-01)
    let rent_rule = rec_service
        .create_rule(CreateRecurringInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_rent.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(2000.00)).unwrap(),
            description: "Aluguel Mensal".to_string(),
            frequency: RecurringFrequency::Monthly,
            day_of_month: Some(31),
            day_of_week: None,
            start_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            end_date: None,
        })
        .await
        .unwrap();

    // 3. Create weekly income: Consultoria R$ 500 toda segunda-feira (day 1, start 2026-03-01)
    let weekly_rule = rec_service
        .create_rule(CreateRecurringInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_salary.name.clone(),
            kind: TransactionKind::Income,
            amount: Money::new(dec!(500.00)).unwrap(),
            description: "Consultoria Semanal".to_string(),
            frequency: RecurringFrequency::Weekly,
            day_of_month: None,
            day_of_week: Some(1), // Monday
            start_date: Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
            end_date: None,
        })
        .await
        .unwrap();

    // 4. Test Dry-Run up to 2026-03-31
    let dry_run_summary = rec_service
        .run_recurring(RunRecurringInput {
            user_id,
            until_date: Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()),
            dry_run: true,
        })
        .await
        .unwrap();

    assert!(dry_run_summary.dry_run);
    assert_eq!(dry_run_summary.rules_evaluated, 2);
    // Rent occurrences: Jan 31, Feb 28 (D-05), Mar 31 -> 3 occurrences
    // Weekly occurrences in Mar 2026 (Mondays): Mar 2, Mar 9, Mar 16, Mar 23, Mar 30 -> 5 occurrences
    assert_eq!(dry_run_summary.transactions_generated.len(), 8);

    // Verify dry run did NOT save any transaction to the database
    let txs_after_dry_run = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            from_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            to_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(txs_after_dry_run.len(), 0);

    // 5. Test Real Run up to 2026-03-31 (Catch-up 3 months)
    let run_summary = rec_service
        .run_recurring(RunRecurringInput {
            user_id,
            until_date: Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()),
            dry_run: false,
        })
        .await
        .unwrap();

    assert!(!run_summary.dry_run);
    assert_eq!(run_summary.transactions_generated.len(), 8);

    // Verify all 8 transactions are saved as pending in database
    let all_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            from_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            to_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            status: Some(TransactionStatus::Pending),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(all_txs.len(), 8);

    // Verify rent occurrences dates (including short month Feb 28)
    let rent_txs: Vec<_> = all_txs
        .iter()
        .filter(|t| t.recurring_rule_id == Some(rent_rule.id.as_uuid()))
        .collect();
    assert_eq!(rent_txs.len(), 3);
    assert_eq!(
        rent_txs[0].date,
        NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
    );
    assert_eq!(
        rent_txs[1].date,
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
    assert_eq!(
        rent_txs[2].date,
        NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
    );
    assert!(rent_txs
        .iter()
        .all(|t| t.status == TransactionStatus::Pending));

    // Verify weekly occurrences
    let weekly_txs: Vec<_> = all_txs
        .iter()
        .filter(|t| t.recurring_rule_id == Some(weekly_rule.id.as_uuid()))
        .collect();
    assert_eq!(weekly_txs.len(), 5);

    // 6. Test IDEMPOTENCY: running again for the same date generates 0 new transactions
    let rerun_summary = rec_service
        .run_recurring(RunRecurringInput {
            user_id,
            until_date: Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()),
            dry_run: false,
        })
        .await
        .unwrap();

    assert_eq!(rerun_summary.transactions_generated.len(), 0);

    let txs_count_after_rerun = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            from_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            to_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(txs_count_after_rerun.len(), 8);

    // 7. Test Incremental Run: advance to next month (2026-04-30)
    let next_month_summary = rec_service
        .run_recurring(RunRecurringInput {
            user_id,
            until_date: Some(NaiveDate::from_ymd_opt(2026, 4, 30).unwrap()),
            dry_run: false,
        })
        .await
        .unwrap();

    // Rent: Apr 30 (1)
    // Weekly: Apr 6, 13, 20, 27 (4)
    // Total new = 5
    assert_eq!(next_month_summary.transactions_generated.len(), 5);

    let total_txs = tx_service
        .list_transactions(ListTransactionsInput {
            user_id,
            from_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            to_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(total_txs.len(), 13);
}
