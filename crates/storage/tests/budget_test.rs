use app::{
    AccountService, BudgetService, CategoryService, CreateTransactionInput, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, BudgetIndicator, Money, TransactionKind};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_budget_workflow_hierarchy_and_statuses() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let budget_service = BudgetService::new(pool);

    // 1. Setup account and categories (parent: Alimentação, children: Mercado, Restaurante)
    let account = acc_service
        .create_account(
            user_id,
            "Nubank".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    let cat_alimentacao = cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_mercado = cat_service
        .create_category(
            user_id,
            "Mercado".to_string(),
            TransactionKind::Expense,
            Some("Alimentação"),
        )
        .await
        .unwrap();

    let cat_transporte = cat_service
        .create_category(
            user_id,
            "Transporte".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let _cat_salario = cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    // 2. Reject budget on Income category
    let err_income_budget = budget_service
        .set_budget(
            user_id,
            "Salário".to_string(),
            Money::new(dec!(1000.00)).unwrap(),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        err_income_budget
            .to_string()
            .contains("apenas para categorias de despesa")
            || err_income_budget.to_string().contains("despesa")
    );

    // 3. Set recurring budget on parent category 'Alimentação': R$ 1.000,00
    budget_service
        .set_budget(
            user_id,
            "Alimentação".to_string(),
            Money::new(dec!(1000.00)).unwrap(),
            None,
        )
        .await
        .unwrap();

    // Set recurring budget on 'Transporte': R$ 500,00
    budget_service
        .set_budget(
            user_id,
            "Transporte".to_string(),
            Money::new(dec!(500.00)).unwrap(),
            None,
        )
        .await
        .unwrap();

    // Set monthly exception for 'Alimentação' in 2026-12: R$ 1.500,00 (Fim de ano)
    budget_service
        .set_budget(
            user_id,
            "Alimentação".to_string(),
            Money::new(dec!(1500.00)).unwrap(),
            Some("2026-12".to_string()),
        )
        .await
        .unwrap();

    // 4. Test List budgets
    let all_budgets = budget_service.list_budgets(user_id, None).await.unwrap();
    assert_eq!(all_budgets.len(), 3);

    // 5. Test Status before expenses (all OK at 0%)
    let status_oct_0 = budget_service
        .get_budget_status(user_id, "2026-10".to_string())
        .await
        .unwrap();
    assert_eq!(status_oct_0.len(), 2);
    for s in &status_oct_0 {
        assert_eq!(s.indicator, BudgetIndicator::Ok);
        assert_eq!(s.consumed_amount, Money::ZERO);
    }

    // 6. Add expenses in October:
    // Expense in subcategory 'Mercado' R$ 500,00 -> 50% of Alimentação (State: OK)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_mercado.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            description: "Supermercado".to_string(),
            status: None,
        })
        .await
        .unwrap();

    let alert_50 = budget_service
        .check_budget_after_expense(
            user_id,
            cat_mercado.id,
            NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        )
        .await
        .unwrap();
    assert!(alert_50.is_none()); // At 50%, no alert

    // Expense in 'Transporte' R$ 420,00 -> 84% of Transporte (State: Warning >= 80%)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_transporte.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(420.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
            description: "Combustível".to_string(),
            status: None,
        })
        .await
        .unwrap();

    let alert_84 = budget_service
        .check_budget_after_expense(
            user_id,
            cat_transporte.id,
            NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        )
        .await
        .unwrap();
    assert!(alert_84.is_some());
    let alert_transporte = alert_84.unwrap();
    assert_eq!(alert_transporte.indicator, BudgetIndicator::Warning);
    assert_eq!(
        alert_transporte.consumed_amount,
        Money::new(dec!(420.00)).unwrap()
    );

    // Expense in 'Alimentação' R$ 600,00 -> Total Alimentação: 500 + 600 = 1.100,00 (110% -> State: Exceeded)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: account.name.clone(),
            category_query: cat_alimentacao.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(600.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
            description: "Jantar Especial".to_string(),
            status: None,
        })
        .await
        .unwrap();

    let alert_110 = budget_service
        .check_budget_after_expense(
            user_id,
            cat_alimentacao.id,
            NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
        )
        .await
        .unwrap();
    assert!(alert_110.is_some());
    let alert_alim = alert_110.unwrap();
    assert_eq!(alert_alim.indicator, BudgetIndicator::Exceeded);
    assert_eq!(
        alert_alim.consumed_amount,
        Money::new(dec!(1100.00)).unwrap()
    );

    // 7. Verify Status summary for October
    let status_oct = budget_service
        .get_budget_status(user_id, "2026-10".to_string())
        .await
        .unwrap();

    assert_eq!(status_oct.len(), 2);
    let alim_status = status_oct
        .iter()
        .find(|s| s.category_name == "Alimentação")
        .unwrap();
    assert_eq!(
        alim_status.budget_amount,
        Money::new(dec!(1000.00)).unwrap()
    );
    assert_eq!(
        alim_status.consumed_amount,
        Money::new(dec!(1100.00)).unwrap()
    );
    assert_eq!(alim_status.remaining_amount, dec!(-100.00));
    assert_eq!(alim_status.percentage, dec!(110.00));
    assert_eq!(alim_status.indicator, BudgetIndicator::Exceeded);

    let transp_status = status_oct
        .iter()
        .find(|s| s.category_name == "Transporte")
        .unwrap();
    assert_eq!(
        transp_status.budget_amount,
        Money::new(dec!(500.00)).unwrap()
    );
    assert_eq!(
        transp_status.consumed_amount,
        Money::new(dec!(420.00)).unwrap()
    );
    assert_eq!(transp_status.remaining_amount, dec!(80.00));
    assert_eq!(transp_status.percentage, dec!(84.00));
    assert_eq!(transp_status.indicator, BudgetIndicator::Warning);

    // 8. Verify Monthly exception in December (Alimentação budget should be 1.500,00)
    let status_dec = budget_service
        .get_budget_status(user_id, "2026-12".to_string())
        .await
        .unwrap();
    let alim_dec = status_dec
        .iter()
        .find(|s| s.category_name == "Alimentação")
        .unwrap();
    assert_eq!(alim_dec.budget_amount, Money::new(dec!(1500.00)).unwrap());
    assert!(alim_dec.is_monthly_exception);
}
